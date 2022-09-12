//! The zcash blockchain and proxied transformations derived from it, do not contain any public
//! information about transaction recipients. Therefore clients must attempt to decrypt each
//! note with each of their keys to determine if they are the recipient.
//! This process is called: `trial_decryption`.

use crate::{
    compact_formats::{CompactBlock, CompactTx},
    grpc_connector::GrpcConnector,
    wallet::{
        data::{ChannelNullifier, TransactionMetadata},
        keys::Keys,
        traits::{
            CompactOutput as _, DomainWalletExt, NoteAndMetadata as _, Nullifier as _,
            WalletKey as _,
        },
        transactions::TransactionMetadataSet,
        MemoDownloadOption,
    },
};
use futures::{stream::FuturesUnordered, StreamExt};
use log::info;
use orchard::{keys::IncomingViewingKey as OrchardIvk, note_encryption::OrchardDomain};
use std::sync::Arc;
use tokio::{
    sync::{
        mpsc::{unbounded_channel, UnboundedSender},
        oneshot, RwLock,
    },
    task::JoinHandle,
};
use zcash_note_encryption::Domain;
use zcash_primitives::{
    consensus::{BlockHeight, Parameters},
    sapling::{note_encryption::SaplingDomain, SaplingIvk},
    transaction::{Transaction, TxId},
};

use super::syncdata::BlazeSyncData;

pub struct TrialDecryptions {
    keys: Arc<RwLock<Keys>>,
    transaction_metadata_set: Arc<RwLock<TransactionMetadataSet>>,
}

impl TrialDecryptions {
    pub fn new(
        keys: Arc<RwLock<Keys>>,
        transaction_metadata_set: Arc<RwLock<TransactionMetadataSet>>,
    ) -> Self {
        Self {
            keys,
            transaction_metadata_set,
        }
    }

    /// Pass keys and data store to dedicated trial_decrpytion *management* thread,
    /// the *management* thread in turns spawns per-1000-cb trial decryption threads.
    pub async fn start(
        &self,
        bsync_data: Arc<RwLock<BlazeSyncData>>,
        detected_transaction_id_sender: UnboundedSender<(
            TxId,
            ChannelNullifier,
            BlockHeight,
            Option<u32>,
        )>,
        transaction_size_filter: Option<u32>,
        full_transaction_fetcher: UnboundedSender<(
            TxId,
            oneshot::Sender<Result<Transaction, String>>,
        )>,
    ) -> (JoinHandle<()>, UnboundedSender<CompactBlock>) {
        //info!("Starting trial decrptions processor");

        // Create a new channel where we'll receive the blocks
        let (transmitter, mut receiver) = unbounded_channel::<CompactBlock>();

        let keys = self.keys.clone();
        let transaction_metadata_set = self.transaction_metadata_set.clone();

        let management_thread_handle = tokio::spawn(async move {
            let mut workers = FuturesUnordered::new();
            let mut cbs = vec![];

            let sapling_ivks = Arc::new(
                keys.read()
                    .await
                    .zkeys
                    .iter()
                    .map(|zk| zk.extfvk().fvk.vk.ivk())
                    .collect::<Vec<_>>(),
            );
            let orchard_ivks = Arc::new(keys.read().await.get_all_orchard_keys_of_type());

            while let Some(cb) = receiver.recv().await {
                cbs.push(cb);

                if cbs.len() >= 1_000 {
                    // We seem to have enough to delegate to a new thread.
                    // Why 1000?
                    let keys = keys.clone();
                    let sapling_ivks = sapling_ivks.clone();
                    let orchard_ivks = orchard_ivks.clone();
                    let transaction_metadata_set = transaction_metadata_set.clone();
                    let bsync_data = bsync_data.clone();
                    let detected_transaction_id_sender = detected_transaction_id_sender.clone();

                    workers.push(tokio::spawn(Self::trial_decrypt_batch(
                        cbs.split_off(0), // This allocates all received cbs to the spawn.
                        keys,
                        bsync_data,
                        sapling_ivks,
                        orchard_ivks,
                        transaction_metadata_set,
                        transaction_size_filter,
                        detected_transaction_id_sender,
                        full_transaction_fetcher.clone(),
                    )));
                }
            }

            // Finish off the remaining < 1000 cbs
            workers.push(tokio::spawn(Self::trial_decrypt_batch(
                cbs,
                keys,
                bsync_data,
                sapling_ivks,
                orchard_ivks,
                transaction_metadata_set,
                transaction_size_filter,
                detected_transaction_id_sender,
                full_transaction_fetcher,
            )));

            while let Some(r) = workers.next().await {
                r.unwrap().unwrap();
            }
        });

        return (management_thread_handle, transmitter);
    }

    /// Trial decryption is invoked by spend-or-view key holders to detect
    /// notes addressed to shielded addresses derived from the spend key.
    /// In the case that the User has requested Memos, and transaction_metadata
    /// remains false throughout domain-specific decryptions, a memo-specific
    /// thread is spawned.
    async fn trial_decrypt_batch(
        compact_blocks: Vec<CompactBlock>,
        keys: Arc<RwLock<Keys>>,
        bsync_data: Arc<RwLock<BlazeSyncData>>,
        sapling_ivks: Arc<Vec<SaplingIvk>>,
        orchard_ivks: Arc<Vec<OrchardIvk>>,
        transaction_metadata_set: Arc<RwLock<TransactionMetadataSet>>,
        transaction_size_filter: Option<u32>,
        detected_transaction_id_sender: UnboundedSender<(
            TxId,
            ChannelNullifier,
            BlockHeight,
            Option<u32>,
        )>,
        full_transaction_fetcher: UnboundedSender<(
            TxId,
            oneshot::Sender<Result<Transaction, String>>,
        )>,
    ) -> Result<(), String> {
        let config = keys.read().await.config().clone();
        let blk_count = compact_blocks.len();
        let mut workers = FuturesUnordered::new();

        let download_memos = bsync_data.read().await.wallet_options.download_memos;

        for compact_block in compact_blocks {
            let height = BlockHeight::from_u32(compact_block.height as u32);
            info!("trial decrypting block {}", height);
            let uri = config.server.read().unwrap().clone();
            let tree_state = GrpcConnector::get_trees(uri, compact_block.height).await?;
            RwLock::write(&bsync_data.read().await.block_data.verification_list)
                .await
                .push(tree_state);

            for (transaction_num, compact_transaction) in compact_block.vtx.iter().enumerate() {
                if let Some(filter) = transaction_size_filter {
                    if compact_transaction.outputs.len() + compact_transaction.actions.len()
                        > filter as usize
                    {
                        break;
                    }
                }
                let mut transaction_metadata = false;

                Self::trial_decrypt_domain_specific_outputs::<SaplingDomain<zingoconfig::Network>>(
                    &mut transaction_metadata,
                    &compact_transaction,
                    transaction_num,
                    &compact_block,
                    sapling_ivks.to_vec(),
                    height,
                    &config,
                    &keys,
                    &bsync_data,
                    &transaction_metadata_set,
                    &detected_transaction_id_sender,
                    &workers,
                );

                Self::trial_decrypt_domain_specific_outputs::<OrchardDomain>(
                    &mut transaction_metadata,
                    &compact_transaction,
                    transaction_num,
                    &compact_block,
                    orchard_ivks.to_vec(),
                    height,
                    &config,
                    &keys,
                    &bsync_data,
                    &transaction_metadata_set,
                    &detected_transaction_id_sender,
                    &workers,
                );

                // Check option to see if we are fetching all transactions.
                // grabs all memos regardless of decryption status.
                // Note the memos are immediately discarded.
                // Perhaps this obfuscates the memos of interest?
                if !transaction_metadata && download_memos == MemoDownloadOption::AllMemos {
                    let transaction_id = TransactionMetadata::new_txid(&compact_transaction.hash);
                    let (transmitter, receiver) = oneshot::channel();
                    full_transaction_fetcher
                        .send((transaction_id, transmitter))
                        .unwrap();

                    workers.push(tokio::spawn(async move {
                        // Discard the result, because this was not a wallet transaction.
                        receiver.await.unwrap().map(|_r| ())
                    }));
                }
            }
        }

        while let Some(r) = workers.next().await {
            r.map_err(|e| e.to_string())??;
        }

        // Update sync status
        bsync_data
            .read()
            .await
            .sync_status
            .write()
            .await
            .trial_dec_done += blk_count as u64;

        // Return a nothing-value
        Ok::<(), String>(())
    }
    fn trial_decrypt_domain_specific_outputs<D>(
        transaction_metadata: &mut bool,
        compact_transaction: &CompactTx,
        transaction_num: usize,
        compact_block: &CompactBlock,
        ivks: Vec<D::IncomingViewingKey>,
        height: BlockHeight,
        config: &zingoconfig::ZingoConfig,
        keys: &Arc<RwLock<Keys>>,
        bsync_data: &Arc<RwLock<BlazeSyncData>>,
        transaction_metadata_set: &Arc<RwLock<TransactionMetadataSet>>,
        detected_transaction_id_sender: &UnboundedSender<(
            TxId,
            ChannelNullifier,
            BlockHeight,
            Option<u32>,
        )>,
        workers: &FuturesUnordered<JoinHandle<Result<(), String>>>,
    ) where
        D: DomainWalletExt<zingoconfig::Network>,
        <D as Domain>::Recipient: crate::wallet::traits::Recipient + Send + 'static,
        <D as Domain>::Note: PartialEq + Send + 'static + Clone,
        [u8; 32]: From<<D as Domain>::ExtractedCommitmentBytes>,
    {
        let outputs = D::CompactOutput::from_compact_transaction(&compact_transaction)
            .into_iter()
            .map(|output| (output.domain(config.chain, height), output.clone()))
            .collect::<Vec<_>>();
        let maybe_decrypted_outputs =
            zcash_note_encryption::batch::try_compact_note_decryption(&ivks, &outputs);
        for maybe_decrypted_output in maybe_decrypted_outputs.into_iter().enumerate() {
            if let (i, Some(((note, to), ivk_num))) = maybe_decrypted_output {
                *transaction_metadata = true; // i.e. we got metadata

                let keys = keys.clone();
                let bsync_data = bsync_data.clone();
                let transaction_metadata_set = transaction_metadata_set.clone();
                let detected_transaction_id_sender = detected_transaction_id_sender.clone();
                let timestamp = compact_block.time as u64;
                let compact_transaction = compact_transaction.clone();
                let config = config.clone();

                workers.push(tokio::spawn(async move {
                    let keys = keys.read().await;
                    let wallet_key = &D::Key::get_keys(&*keys)[ivk_num];
                    let fvk = wallet_key.fvk().unwrap();
                    let have_spending_key = wallet_key.sk().is_some();
                    let uri = bsync_data.read().await.uri().clone();

                    // Get the witness for the note
                    let witness = bsync_data
                        .read()
                        .await
                        .block_data
                        .get_note_witness::<D>(
                            uri,
                            height,
                            transaction_num,
                            i,
                            config.chain.activation_height(D::NU).unwrap().into(),
                        )
                        .await?;

                    let transaction_id = TransactionMetadata::new_txid(&compact_transaction.hash);
                    let nullifier = D::WalletNote::get_nullifier_from_note_fvk_and_witness_position(
                        &note,
                        &fvk,
                        witness.position() as u64,
                    );

                    transaction_metadata_set.write().await.add_new_note::<D>(
                        transaction_id.clone(),
                        height,
                        false,
                        timestamp,
                        note,
                        to,
                        &fvk,
                        have_spending_key,
                        witness,
                    );

                    info!("Trial decrypt Detected txid {}", &transaction_id);

                    detected_transaction_id_sender
                        .send((
                            transaction_id,
                            nullifier.to_channel_nullifier(),
                            height,
                            Some((i) as u32),
                        ))
                        .unwrap();

                    Ok::<_, String>(())
                }));
            }
        }
    }
}

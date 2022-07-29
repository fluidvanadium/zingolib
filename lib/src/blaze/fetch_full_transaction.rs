use crate::wallet::{
    data::OutgoingTxMetadata,
    keys::{Keys, ToBase58Check},
    traits::{
        self as zingo_traits, Bundle as _, DomainWalletExt, NoteData as _, Recipient as _,
        ShieldedOutputExt as _, Spend as _, ToBytes as _, UnspentFromWalletTxns as _,
        WalletKey as _,
    },
    transactions::WalletTxns,
};

use futures::{future::join_all, stream::FuturesUnordered, StreamExt};
use log::info;
use orchard::note_encryption::OrchardDomain;
use std::{
    collections::HashSet,
    convert::TryInto,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
use tokio::{
    sync::{
        mpsc::{unbounded_channel, UnboundedSender},
        oneshot, RwLock,
    },
    task::JoinHandle,
};
use zcash_note_encryption::{try_note_decryption, try_output_recovery_with_ovk};

use zcash_primitives::{
    consensus::BlockHeight,
    legacy::TransparentAddress,
    memo::{Memo, MemoBytes},
    sapling::note_encryption::SaplingDomain,
    transaction::{Transaction, TxId},
};

use super::syncdata::BlazeSyncData;
use zingoconfig::{Network, ZingoConfig};

pub struct FetchFullTxns {
    config: ZingoConfig,
    keys: Arc<RwLock<Keys>>,
    wallet_txns: Arc<RwLock<WalletTxns>>,
}

impl FetchFullTxns {
    pub fn new(
        config: &ZingoConfig,
        keys: Arc<RwLock<Keys>>,
        wallet_txns: Arc<RwLock<WalletTxns>>,
    ) -> Self {
        Self {
            config: config.clone(),
            keys,
            wallet_txns,
        }
    }

    pub async fn start(
        &self,
        fulltx_fetcher: UnboundedSender<(TxId, oneshot::Sender<Result<Transaction, String>>)>,
        bsync_data: Arc<RwLock<BlazeSyncData>>,
    ) -> (
        JoinHandle<Result<(), String>>,
        UnboundedSender<(TxId, BlockHeight)>,
        UnboundedSender<(Transaction, BlockHeight)>,
    ) {
        let wallet_transactions = self.wallet_txns.clone();
        let keys = self.keys.clone();
        let config = self.config.clone();

        let start_height = bsync_data.read().await.sync_status.read().await.start_block;
        let end_height = bsync_data.read().await.sync_status.read().await.end_block;

        let bsync_data_i = bsync_data.clone();

        let (transaction_id_transmitter, mut transaction_id_receiver) =
            unbounded_channel::<(TxId, BlockHeight)>();
        let h1: JoinHandle<Result<(), String>> = tokio::spawn(async move {
            let last_progress = Arc::new(AtomicU64::new(0));
            let mut workers = FuturesUnordered::new();

            while let Some((transaction_id, height)) = transaction_id_receiver.recv().await {
                let config = config.clone();
                let keys = keys.clone();
                let wallet_transactions = wallet_transactions.clone();
                let block_time = bsync_data_i
                    .read()
                    .await
                    .block_data
                    .get_block_timestamp(&height)
                    .await;
                let full_transaction_fetcher = fulltx_fetcher.clone();
                let bsync_data = bsync_data_i.clone();
                let last_progress = last_progress.clone();

                workers.push(tokio::spawn(async move {
                    // It is possible that we recieve the same txid multiple times, so we keep track of all the txids that were fetched
                    let transaction = {
                        // Fetch the TxId from LightwalletD and process all the parts of it.
                        let (transmitter, receiver) = oneshot::channel();
                        full_transaction_fetcher
                            .send((transaction_id, transmitter))
                            .unwrap();
                        receiver.await.unwrap()?
                    };

                    let progress = start_height - u64::from(height);
                    if progress > last_progress.load(Ordering::SeqCst) {
                        bsync_data
                            .read()
                            .await
                            .sync_status
                            .write()
                            .await
                            .txn_scan_done = progress;
                        last_progress.store(progress, Ordering::SeqCst);
                    }

                    Self::scan_full_tx(
                        config,
                        transaction,
                        height,
                        false,
                        block_time,
                        keys,
                        wallet_transactions,
                        None,
                    )
                    .await;

                    Ok::<_, String>(())
                }));
            }

            while let Some(r) = workers.next().await {
                r.map_err(|r| r.to_string())??;
            }

            bsync_data_i
                .read()
                .await
                .sync_status
                .write()
                .await
                .txn_scan_done = start_height - end_height + 1;
            //info!("Finished fetching all full transactions");

            Ok(())
        });

        let wallet_transactions = self.wallet_txns.clone();
        let keys = self.keys.clone();
        let config = self.config.clone();

        let (transaction_transmitter, mut transaction_receiver) =
            unbounded_channel::<(Transaction, BlockHeight)>();

        let h2: JoinHandle<Result<(), String>> = tokio::spawn(async move {
            let bsync_data = bsync_data.clone();

            while let Some((transaction, height)) = transaction_receiver.recv().await {
                let config = config.clone();
                let keys = keys.clone();
                let wallet_transactions = wallet_transactions.clone();

                let block_time = bsync_data
                    .read()
                    .await
                    .block_data
                    .get_block_timestamp(&height)
                    .await;
                Self::scan_full_tx(
                    config,
                    transaction,
                    height,
                    false,
                    block_time,
                    keys,
                    wallet_transactions,
                    None,
                )
                .await;
            }

            //info!("Finished full_tx scanning all txns");
            Ok(())
        });

        let h = tokio::spawn(async move {
            join_all(vec![h1, h2])
                .await
                .into_iter()
                .map(|r| r.map_err(|e| format!("{}", e))?)
                .collect::<Result<(), String>>()
        });

        return (h, transaction_id_transmitter, transaction_transmitter);
    }

    pub(crate) async fn scan_full_tx(
        config: ZingoConfig,
        transaction: Transaction,
        height: BlockHeight,
        unconfirmed: bool,
        block_time: u32,
        keys: Arc<RwLock<Keys>>,
        wallet_transactions: Arc<RwLock<WalletTxns>>,
        price: Option<f64>,
    ) {
        // Remember if this is an outgoing Tx. Useful for when we want to grab the outgoing metadata.
        let mut is_outgoing_transaction = false;

        // Collect our t-addresses for easy checking
        let taddrs = keys.read().await.get_all_taddrs();
        let taddrs_set: HashSet<_> = taddrs.iter().map(|t| t.clone()).collect();

        //todo: investigate scanning all bundles simultaneously
        Self::scan_transparent_bundle(
            &config,
            &transaction,
            height,
            unconfirmed,
            block_time,
            &keys,
            &wallet_transactions,
            &mut is_outgoing_transaction,
            &taddrs_set,
        )
        .await;

        let mut outgoing_metadatas = vec![];
        Self::scan_sapling_bundle(
            &config,
            &transaction,
            height,
            unconfirmed,
            block_time,
            &keys,
            &wallet_transactions,
            &mut is_outgoing_transaction,
            &mut outgoing_metadatas,
        )
        .await;
        Self::scan_orchard_bundle(
            &config,
            &transaction,
            height,
            unconfirmed,
            block_time,
            &keys,
            &wallet_transactions,
            &mut is_outgoing_transaction,
            &mut outgoing_metadatas,
        )
        .await;

        // Process t-address outputs
        // If this transaction in outgoing, i.e., we recieved sent some money in this transaction, then we need to grab all transparent outputs
        // that don't belong to us as the outgoing metadata
        if wallet_transactions
            .read()
            .await
            .total_funds_spent_in(&transaction.txid())
            > 0
        {
            is_outgoing_transaction = true;
        }

        if is_outgoing_transaction {
            if let Some(t_bundle) = transaction.transparent_bundle() {
                for vout in &t_bundle.vout {
                    let taddr = keys
                        .read()
                        .await
                        .address_from_pubkeyhash(vout.script_pubkey.address());

                    if taddr.is_some() && !taddrs_set.contains(taddr.as_ref().unwrap()) {
                        outgoing_metadatas.push(OutgoingTxMetadata {
                            address: taddr.unwrap(),
                            value: vout.value.into(),
                            memo: Memo::Empty,
                        });
                    }
                }
            }

            // Also, if this is an outgoing transaction, then mark all the *incoming* sapling notes to this transaction as change.
            // Note that this is also done in `WalletTxns::add_new_spent`, but that doesn't take into account transparent spends,
            // so we'll do it again here.
            wallet_transactions
                .write()
                .await
                .check_notes_mark_change(&transaction.txid());
        }

        if !outgoing_metadatas.is_empty() {
            wallet_transactions
                .write()
                .await
                .add_outgoing_metadata(&transaction.txid(), outgoing_metadatas);
        }

        // Update price if available
        if price.is_some() {
            wallet_transactions
                .write()
                .await
                .set_price(&transaction.txid(), price);
        }

        //info!("Finished Fetching full transaction {}", tx.txid());
    }

    async fn scan_transparent_bundle(
        config: &ZingoConfig,
        transaction: &Transaction,
        height: BlockHeight,
        unconfirmed: bool,
        block_time: u32,
        keys: &Arc<RwLock<Keys>>,
        wallet_transactions: &Arc<RwLock<WalletTxns>>,
        is_outgoing_transaction: &mut bool,
        taddrs_set: &HashSet<String>,
    ) {
        // Scan all transparent outputs to see if we recieved any money
        if let Some(t_bundle) = transaction.transparent_bundle() {
            for (n, vout) in t_bundle.vout.iter().enumerate() {
                match vout.script_pubkey.address() {
                    Some(TransparentAddress::PublicKey(hash)) => {
                        let output_taddr =
                            hash.to_base58check(&config.base58_pubkey_address(), &[]);
                        if taddrs_set.contains(&output_taddr) {
                            // This is our address. Add this as an output to the txid
                            wallet_transactions.write().await.add_new_taddr_output(
                                transaction.txid(),
                                output_taddr.clone(),
                                height.into(),
                                unconfirmed,
                                block_time as u64,
                                &vout,
                                n as u32,
                            );

                            // Ensure that we add any new HD addresses
                            keys.write().await.ensure_hd_taddresses(&output_taddr);
                        }
                    }
                    _ => {}
                }
            }
        }

        // Scan transparent spends

        // Scan all the inputs to see if we spent any transparent funds in this tx
        let mut total_transparent_value_spent = 0;
        let mut spent_utxos = vec![];

        {
            let current = &wallet_transactions.read().await.current;
            if let Some(t_bundle) = transaction.transparent_bundle() {
                for vin in t_bundle.vin.iter() {
                    // Find the prev txid that was spent
                    let prev_transaction_id = TxId::from_bytes(*vin.prevout.hash());
                    let prev_n = vin.prevout.n() as u64;

                    if let Some(wtx) = current.get(&prev_transaction_id) {
                        // One of the tx outputs is a match
                        if let Some(spent_utxo) = wtx
                            .utxos
                            .iter()
                            .find(|u| u.txid == prev_transaction_id && u.output_index == prev_n)
                        {
                            info!(
                                "Spent: utxo from {} was spent in {}",
                                prev_transaction_id,
                                transaction.txid()
                            );
                            total_transparent_value_spent += spent_utxo.value;
                            spent_utxos.push((
                                prev_transaction_id,
                                prev_n as u32,
                                transaction.txid(),
                                height,
                            ));
                        }
                    }
                }
            }
        }

        // Mark all the UTXOs that were spent here back in their original txns.
        for (prev_transaction_id, prev_n, transaction_id, height) in spent_utxos {
            // Mark that this Tx spent some funds
            *is_outgoing_transaction = true;

            wallet_transactions.write().await.mark_txid_utxo_spent(
                prev_transaction_id,
                prev_n,
                transaction_id,
                height.into(),
            );
        }

        // If this transaction spent value, add the spent amount to the TxID
        if total_transparent_value_spent > 0 {
            *is_outgoing_transaction = true;

            wallet_transactions.write().await.add_taddr_spent(
                transaction.txid(),
                height,
                unconfirmed,
                block_time as u64,
                total_transparent_value_spent,
            );
        }
    }
    async fn scan_sapling_bundle(
        config: &ZingoConfig,
        transaction: &Transaction,
        height: BlockHeight,
        unconfirmed: bool,
        block_time: u32,
        keys: &Arc<RwLock<Keys>>,
        wallet_transactions: &Arc<RwLock<WalletTxns>>,
        is_outgoing_transaction: &mut bool,
        outgoing_metadatas: &mut Vec<OutgoingTxMetadata>,
    ) {
        scan_bundle::<SaplingDomain<Network>>(
            config,
            transaction,
            height,
            unconfirmed,
            block_time,
            keys,
            wallet_transactions,
            is_outgoing_transaction,
            outgoing_metadatas,
        )
        .await
    }
    async fn scan_orchard_bundle(
        config: &ZingoConfig,
        transaction: &Transaction,
        height: BlockHeight,
        unconfirmed: bool,
        block_time: u32,
        keys: &Arc<RwLock<Keys>>,
        wallet_transactions: &Arc<RwLock<WalletTxns>>,
        is_outgoing_transaction: &mut bool,
        outgoing_metadatas: &mut Vec<OutgoingTxMetadata>,
    ) {
        scan_bundle::<OrchardDomain>(
            config,
            transaction,
            height,
            unconfirmed,
            block_time,
            keys,
            wallet_transactions,
            is_outgoing_transaction,
            outgoing_metadatas,
        )
        .await;
    }
}

async fn scan_bundle<D>(
    config: &ZingoConfig,
    transaction: &Transaction,
    height: BlockHeight,
    unconfirmed: bool,
    block_time: u32,
    keys: &Arc<RwLock<Keys>>,
    wallet_transactions: &Arc<RwLock<WalletTxns>>,
    is_outgoing_transaction: &mut bool,
    outgoing_metadatas: &mut Vec<OutgoingTxMetadata>,
) where
    D: zingo_traits::DomainWalletExt<Network>,
    D::Note: Clone + PartialEq,
    D::OutgoingViewingKey: std::fmt::Debug,
    D::Recipient: zingo_traits::Recipient,
    for<'a> &'a <<D as DomainWalletExt<Network>>::Bundle as zingo_traits::Bundle<D, Network>>::Spends:
        IntoIterator<
            Item = &'a <<D as DomainWalletExt<Network>>::Bundle as zingo_traits::Bundle<
                D,
                Network,
            >>::Spend,
        >,
    for<'a> &'a <<D as DomainWalletExt<Network>>::Bundle as zingo_traits::Bundle<D, Network>>::Outputs:
        IntoIterator<
            Item = &'a <<D as DomainWalletExt<Network>>::Bundle as zingo_traits::Bundle<
                D,
                Network,
            >>::Output,
        >,
    D::Memo: zingo_traits::ToBytes<512>,
{
    // Check if any of the nullifiers spent in this transaction are ours. We only need this for unconfirmed transactions,
    // because for transactions in the block, we will check the nullifiers from the blockdata
    if unconfirmed {
        let unspent_nullifiers =
            <<D as DomainWalletExt<Network>>::WalletNote as zingo_traits::NoteData>::Nullifier::unspent_from_wallet_txns(
                &*wallet_transactions.read().await,
            );
        for output in <<D as DomainWalletExt<Network>>::Bundle as zingo_traits::Bundle<
            D,
            Network,
        >>::from_transaction(transaction)
        .into_iter()
        .flat_map(|bundle| bundle.spends().into_iter())
        {
            if let Some((nf, value, transaction_id)) = unspent_nullifiers
                .iter()
                .find(|(nf, _, _)| nf == output.nullifier())
            {
                wallet_transactions.write().await.add_new_spent(
                    transaction.txid(),
                    height,
                    unconfirmed,
                    block_time,
                    <<D as DomainWalletExt<Network>>::Bundle as zingo_traits::Bundle<D, Network>>::Spend::wallet_nullifier(nf),
                    *value,
                    *transaction_id,
                );
            }
        }
    }
    let read_keys = keys.read().await;
    let local_keys = D::Key::get_keys(&*read_keys).clone();
    for output in
        <<D as DomainWalletExt<Network>>::Bundle as zingo_traits::Bundle<D, Network>>::from_transaction(
            transaction,
        )
        .into_iter()
        .flat_map(|bundle| bundle.outputs().into_iter())
    {
        for (i, key) in local_keys.iter().enumerate() {
            let (note, to, memo_bytes) = match key.ivk().map(|ivk| {
                try_note_decryption::<
                    D,
                    <<D as DomainWalletExt<Network>>::Bundle as zingo_traits::Bundle<D, Network>>::Output,
                >(&output.domain(height, config.chain), &ivk, &output)
            }) {
                Some(Some(ret)) => ret,
                _ => continue,
            };
            let memo_bytes = MemoBytes::from_bytes(&memo_bytes.to_bytes()).unwrap();
            if unconfirmed {
                wallet_transactions.write().await.add_pending_note::<D>(
                    transaction.txid(),
                    height,
                    block_time as u64,
                    note.clone(),
                    to,
                    &match &local_keys.get(i).unwrap().fvk() {
                        Some(k) => k.clone(),
                        None => continue,
                    },
                );
            }
            let memo = memo_bytes
                .clone()
                .try_into()
                .unwrap_or(Memo::Future(memo_bytes));
            wallet_transactions
                .write()
                .await
                .add_memo_to_note::<D::WalletNote>(&transaction.txid(), note, memo);
        }
        outgoing_metadatas.extend(
            local_keys
                .iter()
                .filter_map(|key| {
                    match try_output_recovery_with_ovk::<
                        D,
                        <<D as DomainWalletExt<Network>>::Bundle as zingo_traits::Bundle<
                            D,
                            Network,
                        >>::Output,
                    >(
                        &output.domain(height, config.chain),
                        &key.ovk().unwrap(),
                        &output,
                        &output.value_commitment(),
                        &output.out_ciphertext(),
                    ) {
                        Some((note, payment_address, memo_bytes)) => {
                            // Mark this transaction as an outgoing transaction, so we can grab all outgoing metadata
                            *is_outgoing_transaction = true;
                            let address = payment_address.encode(config.chain);

                            // Check if this is change, and if it also doesn't have a memo, don't add
                            // to the outgoing metadata.
                            // If this is change (i.e., funds sent to ourself) AND has a memo, then
                            // presumably the users is writing a memo to themself, so we will add it to
                            // the outgoing metadata, even though it might be confusing in the UI, but hopefully
                            // the user can make sense of it.
                            match Memo::from_bytes(&memo_bytes.to_bytes()) {
                                Err(_) => None,
                                Ok(memo) => {
                                    if D::Key::get_addresses(&read_keys).contains(&address)
                                        && memo == Memo::Empty
                                    {
                                        None
                                    } else {
                                        Some(OutgoingTxMetadata {
                                            address,
                                            value: D::WalletNote::value(&note),
                                            memo,
                                        })
                                    }
                                }
                            }
                        }
                        None => None,
                    }
                })
                .collect::<Vec<_>>(),
        );
    }
}

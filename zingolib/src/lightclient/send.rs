//! TODO: Add Mod Description Here!
use log::debug;

use zcash_client_backend::{
    address::Address,
    zip321::{Payment, TransactionRequest},
};
use zcash_primitives::{
    consensus::BlockHeight,
    memo::MemoBytes,
    transaction::{components::amount::NonNegativeAmount, fees::zip317::MINIMUM_FEE},
};
use zcash_proofs::prover::LocalTxProver;

use super::{LightClient, LightWalletSendProgress};
use crate::{utils::zatoshis_from_u64, wallet::Pool};

#[cfg(feature = "zip317")]
use {
    crate::{error::ZingoLibError, wallet::tx_map_and_maybe_trees::TxMapAndMaybeTrees},
    std::{num::NonZeroU32, ops::DerefMut},
    zcash_client_backend::{
        data_api::wallet::input_selection::GreedyInputSelector, ShieldedProtocol,
    },
    zcash_primitives::transaction::TxId,
    zingoconfig::ChainType,
};

#[cfg(feature = "zip317")]
type GISKit = GreedyInputSelector<
    TxMapAndMaybeTrees,
    zcash_client_backend::fees::zip317::SingleOutputChangeStrategy,
>;

/// converts from raw receivers to TransactionRequest
pub fn receivers_becomes_transaction_request(
    receivers: Vec<(Address, NonNegativeAmount, Option<MemoBytes>)>,
) -> Result<TransactionRequest, zcash_client_backend::zip321::Zip321Error> {
    let mut payments = vec![];
    for out in receivers.clone() {
        payments.push(Payment {
            recipient_address: out.0,
            amount: out.1,
            memo: out.2,
            label: None,
            message: None,
            other_params: vec![],
        });
    }

    TransactionRequest::new(payments)
}

impl LightClient {
    async fn get_submission_height(&self) -> Result<BlockHeight, String> {
        Ok(BlockHeight::from_u32(
            crate::grpc_connector::get_latest_block(self.config.get_lightwalletd_uri())
                .await?
                .height as u32,
        ) + 1)
    }

    /// Unstable function to expose the zip317 interface for development
    // TOdo: add correct functionality and doc comments / tests
    // TODO: Add migrate_sapling_to_orchard argument
    #[cfg(feature = "zip317")]
    pub async fn do_propose_spend(
        &self,
        receivers: Vec<(Address, NonNegativeAmount, Option<MemoBytes>)>,
    ) -> Result<crate::data::proposal::TransferProposal, String> {
        let request =
            receivers_becomes_transaction_request(receivers).map_err(|e| e.to_string())?;

        let change_strategy = zcash_client_backend::fees::zip317::SingleOutputChangeStrategy::new(
            zcash_primitives::transaction::fees::zip317::FeeRule::standard(),
            None,
            ShieldedProtocol::Orchard,
        ); // review consider change strategy!

        let input_selector = GISKit::new(
            change_strategy,
            zcash_client_backend::fees::DustOutputPolicy::default(),
        );

        let mut tmamt = self
            .wallet
            .transaction_context
            .transaction_metadata_set
            .write()
            .await;

        let proposal = zcash_client_backend::data_api::wallet::propose_transfer::<
            TxMapAndMaybeTrees,
            ChainType,
            GISKit,
            ZingoLibError,
        >(
            tmamt.deref_mut(),
            &self.wallet.transaction_context.config.chain,
            zcash_primitives::zip32::AccountId::ZERO,
            &input_selector,
            request,
            NonZeroU32::MIN, //review! use custom constant?
        )
        .map_err(|e| ZingoLibError::Error(format!("error this function todo error {:?}", e)))?;

        let mut latest_proposal_lock = self.latest_proposal.write().await;
        *latest_proposal_lock = Some(crate::data::proposal::ZingoProposal::Transfer(
            proposal.clone(),
        ));
        Ok(proposal)
    }

    /// Unstable function to expose the zip317 interface for development
    // TOdo: add correct functionality and doc comments / tests
    #[cfg(feature = "zip317")]
    pub async fn do_propose_shield(
        &self,
        _address_amount_memo_tuples: Vec<(&str, u64, Option<MemoBytes>)>,
    ) -> Result<crate::data::proposal::ShieldProposal, String> {
        todo!()
    }

    /// Unstable function to expose the zip317 interface for development
    // TODO: add correct functionality and doc comments / tests
    #[cfg(feature = "zip317")]
    pub async fn do_send_proposal(&self) -> Result<Vec<TxId>, String> {
        if let Some(proposal) = self.latest_proposal.read().await.as_ref() {
            match proposal {
                crate::lightclient::ZingoProposal::Transfer(_) => {
                    Ok(vec![TxId::from_bytes([1u8; 32])])
                }
                crate::lightclient::ZingoProposal::Shield(_) => {
                    Ok(vec![TxId::from_bytes([222u8; 32])])
                }
            }
        } else {
            Err("No proposal. Call do_propose first.".to_string())
        }
    }

    /// Send funds
    pub async fn do_send(
        &self,
        receivers: Vec<(Address, NonNegativeAmount, Option<MemoBytes>)>,
    ) -> Result<String, String> {
        let transaction_submission_height = self.get_submission_height().await?;
        // First, get the consensus branch ID
        debug!("Creating transaction");

        let _lock = self.sync_lock.lock().await;
        // I am not clear on how long this operation may take, but it's
        // clearly unnecessary in a send that doesn't include sapling
        // TODO: Remove from sends that don't include Sapling
        let (sapling_output, sapling_spend) = self.read_sapling_params()?;

        let sapling_prover = LocalTxProver::from_bytes(&sapling_spend, &sapling_output);

        self.wallet
            .send_to_addresses(
                sapling_prover,
                vec![crate::wallet::Pool::Orchard, crate::wallet::Pool::Sapling], // This policy doesn't allow
                // spend from transparent.
                receivers,
                transaction_submission_height,
                |transaction_bytes| {
                    crate::grpc_connector::send_transaction(
                        self.get_server_uri(),
                        transaction_bytes,
                    )
                },
            )
            .await
    }

    /// TODO: Add Doc Comment Here!
    pub async fn do_send_progress(&self) -> Result<LightWalletSendProgress, String> {
        let progress = self.wallet.get_send_progress().await;
        Ok(LightWalletSendProgress {
            progress: progress.clone(),
            interrupt_sync: *self.interrupt_sync.read().await,
        })
    }

    /// Shield funds. Send transparent or sapling funds to a unified address.
    /// Defaults to the unified address of the capability if `address` is `None`.
    pub async fn do_shield(
        &self,
        pools_to_shield: &[Pool],
        address: Option<Address>,
    ) -> Result<String, String> {
        let transaction_submission_height = self.get_submission_height().await?;
        let fee = u64::from(MINIMUM_FEE); // TODO: This can no longer be hard coded, and must be calced
                                          // as a fn of the transactions structure.
        let tbal = self
            .wallet
            .tbalance(None)
            .await
            .expect("to receive a balance");
        let sapling_bal = self
            .wallet
            .spendable_sapling_balance(None)
            .await
            .unwrap_or(0);

        // Make sure there is a balance, and it is greater than the amount
        let balance_to_shield = if pools_to_shield.contains(&Pool::Transparent) {
            tbal
        } else {
            0
        } + if pools_to_shield.contains(&Pool::Sapling) {
            sapling_bal
        } else {
            0
        };
        if balance_to_shield <= fee {
            return Err(format!(
                "Not enough transparent/sapling balance to shield. Have {} zats, need more than {} zats to cover tx fee",
                balance_to_shield, fee
            ));
        }

        let address = address.unwrap_or(Address::from(
            self.wallet.wallet_capability().addresses()[0].clone(),
        ));
        let amount = zatoshis_from_u64(balance_to_shield - fee)
            .expect("balance cannot be outside valid range of zatoshis");
        let receiver = vec![(address, amount, None)];

        let _lock = self.sync_lock.lock().await;
        let (sapling_output, sapling_spend) = self.read_sapling_params()?;

        let sapling_prover = LocalTxProver::from_bytes(&sapling_spend, &sapling_output);

        self.wallet
            .send_to_addresses(
                sapling_prover,
                pools_to_shield.to_vec(),
                receiver,
                transaction_submission_height,
                |transaction_bytes| {
                    crate::grpc_connector::send_transaction(
                        self.get_server_uri(),
                        transaction_bytes,
                    )
                },
            )
            .await
    }
}

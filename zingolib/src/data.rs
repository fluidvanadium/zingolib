//! This is a mod for data structs that will be used across all sections of zingolib.

use zcash_primitives::transaction::TxId;
pub mod proposal;
pub mod witness_trees;

/// transforming data related to the destination of a send.
pub mod receivers {
    use zcash_client_backend::zip321::Payment;
    use zcash_client_backend::zip321::TransactionRequest;
    use zcash_client_backend::zip321::Zip321Error;
    use zcash_keys::address;
    use zcash_primitives::memo::MemoBytes;
    use zcash_primitives::transaction::components::amount::NonNegativeAmount;

    /// A list of Receivers
    pub type Receivers = Vec<Receiver>;

    /// The superficial representation of the the consumer's intended receiver
    #[derive(Clone, Debug, PartialEq)]
    pub struct Receiver {
        pub(crate) recipient_address: address::Address,
        pub(crate) amount: NonNegativeAmount,
        pub(crate) memo: Option<MemoBytes>,
    }
    impl Receiver {
        /// Create a new Receiver
        pub fn new(
            recipient_address: address::Address,
            amount: NonNegativeAmount,
            memo: Option<MemoBytes>,
        ) -> Self {
            Self {
                recipient_address,
                amount,
                memo,
            }
        }
    }
    impl From<Receiver> for Payment {
        fn from(receiver: Receiver) -> Self {
            Self {
                recipient_address: receiver.recipient_address,
                amount: receiver.amount,
                memo: receiver.memo,
                label: None,
                message: None,
                other_params: vec![],
            }
        }
    }

    /// Creates a [`zcash_client_backend::zip321::TransactionRequest`] from receivers.
    pub fn transaction_request_from_receivers(
        receivers: Receivers,
    ) -> Result<TransactionRequest, Zip321Error> {
        let payments = receivers
            .into_iter()
            .map(|receiver| receiver.into())
            .collect();

        TransactionRequest::new(payments)
    }
}

pub mod pool_enums {
    /// wraps a nullifier from one shielded pool or the other.
    pub enum PoolNullifier {
        /// sapling nullifier
        Sapling(sapling_crypto::Nullifier),
        /// orchard nullifier
        Orchard(orchard::note::Nullifier),
    }
}

pub mod note_id {
    use std::fmt;

    use zcash_primitives::transaction::TxId;

    use crate::wallet::data::PoolNullifier;

    pub struct NoteId {
        txid: TxId,
        pool_nullifier: PoolNullifier,
    }

    // impl fmt::Display for NoteId {
    //     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    //         write!(
    //             f,
    //             "txid {} protocol {:?} output_index {}",
    //             self.txid(),
    //             self.protocol(),
    //             self.output_index()
    //         )
    //     }
    // }
}

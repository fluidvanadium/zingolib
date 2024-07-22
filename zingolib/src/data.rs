//! This is a mod for data structs that will be used across all sections of zingolib.
pub mod proposal;
pub mod witness_trees;

/// transforming data related to the destination of a send.
pub mod receivers {
    use zcash_address::ParseError;
    use zcash_address::ZcashAddress;
    use zcash_client_backend::zip321::Payment;
    use zcash_client_backend::zip321::TransactionRequest;
    use zcash_client_backend::zip321::Zip321Error;
    use zcash_primitives::memo::MemoBytes;
    use zcash_primitives::transaction::components::amount::NonNegativeAmount;

    /// A list of Receivers
    pub type Receivers = Vec<Receiver>;

    /// The superficial representation of the the consumer's intended receiver
    #[derive(Clone, Debug, PartialEq)]
    pub struct Receiver {
        pub(crate) recipient: String,
        pub(crate) amount: NonNegativeAmount,
        pub(crate) memo: Option<MemoBytes>,
    }
    impl Receiver {
        /// Create a new Receiver
        pub fn new(recipient: String, amount: NonNegativeAmount, memo: Option<MemoBytes>) -> Self {
            Self {
                recipient,
                amount,
                memo,
            }
        }
    }

    /// anything that can go wrong parsing a TransactionRequest from a receiver
    #[derive(thiserror::Error, Debug)]
    pub enum ReceiverParseError {
        /// see Debug
        #[error("Could not parse address: {0}")]
        AddressParse(ParseError),
        /// see Debug
        #[error("Cant send memo to transparent receiver")]
        MemoDisallowed,
        /// see Debug
        #[error("Could not build TransactionRequest: {0}")]
        Request(Zip321Error),
    }

    /// Creates a [`zcash_client_backend::zip321::TransactionRequest`] from receivers.
    pub fn transaction_request_from_receivers(
        receivers: Receivers,
    ) -> Result<TransactionRequest, ReceiverParseError> {
        let payments = receivers
            .into_iter()
            .map(|receiver| {
                Payment::new(
                    ZcashAddress::try_from_encoded(receiver.recipient.as_str())
                        .map_err(ReceiverParseError::AddressParse)?,
                    receiver.amount,
                    receiver.memo,
                    None,
                    None,
                    vec![],
                )
                .ok_or(ReceiverParseError::MemoDisallowed)
            })
            .collect::<Result<Vec<Payment>, ReceiverParseError>>()?;

        TransactionRequest::new(payments).map_err(ReceiverParseError::Request)
    }
}

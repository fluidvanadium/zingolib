//! Notes have a lifecycle made up of discrete states, that is a note is *always* in exactly one
//! of these states.
use zcash_primitives::{consensus::BlockHeight, transaction::TxId};

/// A 32 wide bitmask with 0 in the last 5 places
pub const BLOCKHEIGHT_PLACEHOLDER_LOCAL: u32 = <u32>::max_value() - (16 + 8 + 4 + 2 + 1);
/// A 32 wide bitmask with 1 in the least significant place, and 0 inn each of the next 4
pub const BLOCKHEIGHT_PLACEHOLDER_INMEMPOOL: u32 = <u32>::max_value() - (16 + 8 + 4 + 2);

/// A 32 wide bitmask with 0 at 2^5, 2^3, 2^2, 2^1, and 2^0
pub const BLOCKHEIGHT_PLACEHOLDER_NOKNOWNSPENDS: u32 = <u32>::max_value() - (32 + 8 + 4 + 2 + 1);
/// A 32 wide bitmask with 0 at 2^5, 2^3, 2^2, and 2^1
pub const BLOCKHEIGHT_PLACEHOLDER_PENDINGSPEND: u32 = <u32>::max_value() - (32 + 8 + 4 + 2);

fn u32_height_or_placeholder(option_blockheight: Option<BlockHeight>) -> u32 {
    match option_blockheight {
        Some(block_height) => u32::from(block_height),
        None => BLOCKHEIGHT_PLACEHOLDER_INMEMPOOL,
    }
}

/// All notes are in one of these three states
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ConfirmationStatus {
    Local,
    /// we may know when it entered the mempool.
    InMempool(Option<BlockHeight>),
    /// confirmed on blockchain implies a height. this data piece will eventually be a block height
    ConfirmedOnChain(BlockHeight),
}

impl ConfirmationStatus {
    pub fn is_in_mempool(&self) -> bool {
        matches!(self, Self::InMempool(_))
    }
    pub fn is_confirmed(&self) -> bool {
        matches!(self, Self::ConfirmedOnChain(_))
    }
    pub fn is_confirmed_at_or_above(&self, height: &BlockHeight) -> bool {
        match self {
            Self::ConfirmedOnChain(block_height) => block_height >= height,
            _ => false,
        }
    }
    pub fn is_confirmed_at_or_below(&self, height: &BlockHeight) -> bool {
        match self {
            Self::ConfirmedOnChain(block_height) => block_height <= height,
            _ => false,
        }
    }
    pub fn is_expired(&self, cutoff: &BlockHeight) -> bool {
        match self {
            Self::Local => true, // Why? What? Local is "expired"? I am skeptical, that seems wrong.
            Self::InMempool(option_blockheight) => match option_blockheight {
                None => true,
                // If the height that the note entered the mempool is less than the
                // "cutoff", doesn't that mean that it's not expired?  If that interpretation
                // is right then we should flip the inequality, if it's not correct, then I
                // am misinterpreting at least one term.
                Some(block_height) => block_height < cutoff,
            },
            Self::ConfirmedOnChain(_) => false,
        }
    }
    // this function and the placeholder is not a preferred pattern. please use match whenever possible.
    //  Is this comment obsolete ---^?
    // If this fn returns a BlockHeight, why is the type u32?
    pub fn get_height_and_is_confirmed(&self) -> (u32, bool) {
        match self {
            Self::Local => (BLOCKHEIGHT_PLACEHOLDER_LOCAL, false),
            Self::InMempool(opt_block) => (u32_height_or_placeholder(*opt_block), false),
            Self::ConfirmedOnChain(block) => (u32::from(*block), true),
        }
    }
    // note, by making unconfirmed the true case, this does a potentially confusing boolean flip
    // I think having more than one way to do essentially the same thing (like an inverse) is
    // a troublesome pattern, because it creates more code for bugs to hide in.
    pub fn get_height_and_is_unconfirmed(&self) -> (u32, bool) {
        match self {
            Self::Local => (BLOCKHEIGHT_PLACEHOLDER_LOCAL, true),
            Self::InMempool(opt_block) => (u32_height_or_placeholder(*opt_block), true),
            Self::ConfirmedOnChain(block) => (u32::from(*block), false),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpendStatus {
    // A note can only be spent once.  Maybe this would be a reasonable place to reuse the
    // "Local" nym?   I guess that if the client in question is not the only client
    // with a spend capability for the note, then the might be actually spent, and the
    // client is unaware..  but this is *only* possible if the spender is non-Local..  right?!
    NoKnown,
    Pending(TxId),
    Confirmed(TxId, BlockHeight),
}

impl SpendStatus {
    pub fn from_txid_and_confirmation(
        spending_txid: TxId,
        confirmation_status: ConfirmationStatus,
    ) -> Self {
        match confirmation_status {
            ConfirmationStatus::Local | ConfirmationStatus::InMempool(_) => {
                Self::Pending(spending_txid)
            }
            ConfirmationStatus::ConfirmedOnChain(confirmation_height) => {
                Self::Confirmed(spending_txid, confirmation_height)
            }
        }
    }
    pub fn from_opt_txidandu32(option_txidandu32: Option<(TxId, u32)>) -> Self {
        match option_txidandu32 {
            None => Self::NoKnown,
            Some((txid, confirmed_height)) => {
                Self::Confirmed(txid, BlockHeight::from_u32(confirmed_height))
            }
        }
    }
    pub fn from_opt_i32_and_option_txid(
        option_height: Option<i32>,
        option_txid: Option<TxId>,
    ) -> Self {
        match option_txid {
            None => Self::NoKnown,
            Some(txid) => match option_height {
                None => Self::Pending(txid),
                Some(integer) => match u32::try_from(integer) {
                    Err(_) => Self::Pending(txid),
                    Ok(height) => Self::Confirmed(txid, BlockHeight::from_u32(height)),
                },
            },
        }
    }
    pub fn is_unspent(&self) -> bool {
        matches!(self, Self::NoKnown)
    }
    pub fn is_pending_spend(&self) -> bool {
        matches!(self, Self::Pending(_))
    }
    pub fn is_pending_spend_or_confirmed_spent(&self) -> bool {
        matches!(self, Self::Pending(_) | Self::Confirmed(_, _))
    }
    pub fn is_confirmed_spent(&self) -> bool {
        matches!(self, Self::Confirmed(_, _))
    }
    pub fn is_not_confirmed_spent(&self) -> bool {
        !matches!(self, Self::Confirmed(_, _))
    }
    pub fn erase_spent_in_txids(&mut self, txids: &[TxId]) {
        match self {
            Self::NoKnown => (),
            Self::Pending(txid) => {
                if txids.contains(txid) {
                    *self = Self::NoKnown;
                }
            }
            Self::Confirmed(txid, _) => {
                if txids.contains(txid) {
                    *self = Self::NoKnown;
                }
            }
        }
    }
    // this function and seperate enum possibilities is not a preferred pattern. please use match whenever possible.
    pub fn get_option_i32_and_option_txid(&self) -> (Option<i32>, Option<TxId>) {
        match self {
            Self::NoKnown => (None, None),
            Self::Pending(_) => (None, None),
            Self::Confirmed(txid, block) => (Some(u32::from(*block) as i32), Some(*txid)),
        }
    }
    pub fn to_opt_txidandu32(&self) -> Option<(TxId, u32)> {
        match self {
            Self::Confirmed(txid, confirmation_height) => {
                Some((*txid, u32::from(*confirmation_height)))
            }
            _ => None,
        }
    }
    pub fn to_serde_json(&self) -> serde_json::Value {
        match self {
            Self::NoKnown => serde_json::Value::from("no known spends"),
            Self::Pending(spent_txid) => serde_json::json!({
                "pending_spend_at_txid": format!("{}",spent_txid),}),
            Self::Confirmed(spent_txid, block_height) => serde_json::json!({
                "spent_at_txid": format!("{}",spent_txid),
                "spend_at_block_height": u32::from(*block_height),}),
        }
    }
}

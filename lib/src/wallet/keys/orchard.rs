use orchard::keys::{FullViewingKey, IncomingViewingKey, OutgoingViewingKey, Scope, SpendingKey};
use zcash_client_backend::address::UnifiedAddress;
// A struct that holds orchard private keys or view keys
#[derive(Clone, Debug, PartialEq)]
pub struct OrchardKey {
    pub(crate) key: WalletOKeyInner,
    locked: bool,
    pub(crate) unified_address: UnifiedAddress,

    // If this is a key derived from our HD seed, the account number of the key
    // This is effectively the index number used to generate the key from the seed
    pub(crate) hdkey_num: Option<u32>,

    // If locked, the encrypted private key is stored here
    enc_key: Option<Vec<u8>>,
    nonce: Option<Vec<u8>>,
}
#[allow(dead_code)]
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) enum WalletOKeyInner {
    HdKey(SpendingKey),
    ImportedSpendingKey(SpendingKey),
    ImportedFullViewKey(FullViewingKey),
    ImportedInViewKey(IncomingViewingKey),
    ImportedOutViewKey(OutgoingViewingKey),
}

impl TryFrom<&WalletOKeyInner> for SpendingKey {
    type Error = String;
    fn try_from(key: &WalletOKeyInner) -> Result<SpendingKey, String> {
        match key {
            WalletOKeyInner::HdKey(k) => Ok(*k),
            WalletOKeyInner::ImportedSpendingKey(k) => Ok(*k),
            other => Err(format!("{other:?} is not a spending key")),
        }
    }
}
impl TryFrom<&WalletOKeyInner> for FullViewingKey {
    type Error = String;
    fn try_from(key: &WalletOKeyInner) -> Result<FullViewingKey, String> {
        match key {
            WalletOKeyInner::HdKey(k) => Ok(FullViewingKey::from(k)),
            WalletOKeyInner::ImportedSpendingKey(k) => Ok(FullViewingKey::from(k)),
            WalletOKeyInner::ImportedFullViewKey(k) => Ok(k.clone()),
            other => Err(format!("{other:?} is not a full viewing key")),
        }
    }
}
impl TryFrom<&WalletOKeyInner> for OutgoingViewingKey {
    type Error = String;
    fn try_from(key: &WalletOKeyInner) -> Result<OutgoingViewingKey, String> {
        match key {
            WalletOKeyInner::ImportedOutViewKey(k) => Ok(k.clone()),
            WalletOKeyInner::ImportedFullViewKey(k) => Ok(k.to_ovk(Scope::External)),
            WalletOKeyInner::ImportedInViewKey(k) => {
                Err(format!("Received ivk {k:?} which does not contain an ovk"))
            }
            _ => Ok(FullViewingKey::try_from(key)
                .unwrap()
                .to_ovk(Scope::External)),
        }
    }
}

impl TryFrom<&WalletOKeyInner> for IncomingViewingKey {
    type Error = String;
    fn try_from(key: &WalletOKeyInner) -> Result<IncomingViewingKey, String> {
        match key {
            WalletOKeyInner::ImportedInViewKey(k) => Ok(k.clone()),
            WalletOKeyInner::ImportedFullViewKey(k) => Ok(k.to_ivk(Scope::External)),
            WalletOKeyInner::ImportedOutViewKey(k) => {
                Err(format!("Received ovk {k:?} which does not contain an ivk"))
            }
            _ => Ok(FullViewingKey::try_from(key)
                .unwrap()
                .to_ivk(Scope::External)),
        }
    }
}

impl PartialEq for WalletOKeyInner {
    fn eq(&self, other: &Self) -> bool {
        use subtle::ConstantTimeEq as _;
        use WalletOKeyInner::*;
        match (self, other) {
            (HdKey(a), HdKey(b)) => bool::from(a.ct_eq(b)),
            (ImportedSpendingKey(a), ImportedSpendingKey(b)) => bool::from(a.ct_eq(b)),
            (ImportedFullViewKey(a), ImportedFullViewKey(b)) => a == b,
            (ImportedInViewKey(a), ImportedInViewKey(b)) => a == b,
            (ImportedOutViewKey(a), ImportedOutViewKey(b)) => a.as_ref() == b.as_ref(),
            _ => false,
        }
    }
}

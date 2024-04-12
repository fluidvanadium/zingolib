use incrementalmerkletree::Position;
use zcash_primitives::{memo::Memo, transaction::TxId};

use super::{
    super::{data::TransactionRecord, Pool},
    NoteInterface, ShieldedNoteInterface,
};

pub struct SaplingNote {
    pub diversifier: sapling_crypto::Diversifier,
    pub note: sapling_crypto::Note,

    // The position of this note's value commitment in the global commitment tree
    // We need to create a witness to it, to spend
    pub(crate) witnessed_position: Option<Position>,

    // The note's index in its containing transaction
    pub(crate) output_index: Option<u32>,

    pub nullifier: Option<sapling_crypto::Nullifier>,

    pub spent: Option<(TxId, u32)>, // If this note was confirmed spent. Todo: as related to unconfirmed spent, this is potential data incoherence

    // If this note was spent in a send, but has not yet been confirmed.
    // Contains the transaction id and height at which it was broadcast
    pub unconfirmed_spent: Option<(TxId, u32)>,
    pub memo: Option<Memo>,
    pub is_change: bool,

    // If the spending key is available in the wallet (i.e., whether to keep witness up-to-date) Todo should this data point really be here?
    pub have_spending_key: bool,
}

impl std::fmt::Debug for SaplingNote {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SaplingNoteData")
            .field("diversifier", &self.diversifier)
            .field("note", &self.note)
            .field("nullifier", &self.nullifier)
            .field("spent", &self.spent)
            .field("unconfirmed_spent", &self.unconfirmed_spent)
            .field("memo", &self.memo)
            .field("diversifier", &self.diversifier)
            .field("note", &self.note)
            .field("nullifier", &self.nullifier)
            .field("spent", &self.spent)
            .field("unconfirmed_spent", &self.unconfirmed_spent)
            .field("memo", &self.memo)
            .field("is_change", &self.is_change)
            .finish_non_exhaustive()
    }
}

impl NoteInterface for SaplingNote {
    fn spent(&self) -> &Option<(TxId, u32)> {
        &self.spent
    }
    fn spent_mut(&mut self) -> &mut Option<(TxId, u32)> {
        &mut self.spent
    }
    fn pending_spent(&self) -> &Option<(TxId, u32)> {
        &self.unconfirmed_spent
    }
    fn pending_spent_mut(&mut self) -> &mut Option<(TxId, u32)> {
        &mut self.unconfirmed_spent
    }
}

impl ShieldedNoteInterface for SaplingNote {
    type Diversifier = sapling_crypto::Diversifier;
    type Note = sapling_crypto::Note;
    type Node = sapling_crypto::Node;
    type Nullifier = sapling_crypto::Nullifier;

    fn diversifier(&self) -> &Self::Diversifier {
        &self.diversifier
    }

    fn nullifier_mut(&mut self) -> &mut Option<Self::Nullifier> {
        &mut self.nullifier
    }

    fn from_parts(
        diversifier: sapling_crypto::Diversifier,
        note: sapling_crypto::Note,
        witnessed_position: Option<Position>,
        nullifier: Option<sapling_crypto::Nullifier>,
        spent: Option<(TxId, u32)>,
        unconfirmed_spent: Option<(TxId, u32)>,
        memo: Option<Memo>,
        is_change: bool,
        have_spending_key: bool,
        output_index: Option<u32>,
    ) -> Self {
        Self {
            diversifier,
            note,
            witnessed_position,
            nullifier,
            spent,
            unconfirmed_spent,
            memo,
            is_change,
            have_spending_key,
            output_index,
        }
    }

    fn get_deprecated_serialized_view_key_buffer() -> Vec<u8> {
        vec![0u8; 169]
    }

    fn have_spending_key(&self) -> bool {
        self.have_spending_key
    }

    fn is_change(&self) -> bool {
        self.is_change
    }

    fn is_change_mut(&mut self) -> &mut bool {
        &mut self.is_change
    }

    fn memo(&self) -> &Option<Memo> {
        &self.memo
    }

    fn memo_mut(&mut self) -> &mut Option<Memo> {
        &mut self.memo
    }

    fn note(&self) -> &Self::Note {
        &self.note
    }

    fn nullifier(&self) -> Option<Self::Nullifier> {
        self.nullifier
    }

    fn pool() -> Pool {
        Pool::Sapling
    }

    fn transaction_metadata_notes(wallet_transaction: &TransactionRecord) -> &Vec<Self> {
        &wallet_transaction.sapling_notes
    }

    fn transaction_metadata_notes_mut(
        wallet_transaction: &mut TransactionRecord,
    ) -> &mut Vec<Self> {
        &mut wallet_transaction.sapling_notes
    }

    fn value_from_note(note: &Self::Note) -> u64 {
        note.value().inner()
    }

    fn witnessed_position(&self) -> &Option<Position> {
        &self.witnessed_position
    }

    fn witnessed_position_mut(&mut self) -> &mut Option<Position> {
        &mut self.witnessed_position
    }

    fn output_index(&self) -> &Option<u32> {
        &self.output_index
    }
    fn to_zcb_note(&self) -> zcash_client_backend::wallet::Note {
        zcash_client_backend::wallet::Note::Sapling(self.note().clone())
    }
}

#[cfg(feature = "test-features")]
pub(crate) mod mocks {
    use incrementalmerkletree::Position;
    use zcash_primitives::{memo::Memo, transaction::TxId};

    use crate::{
        test_framework::mocks::build_method,
        wallet::{notes::ShieldedNoteInterface, traits::FromBytes},
    };

    use super::SaplingNote;

    /// builds a mock transparent note after all pieces are supplied
    pub(crate) struct SaplingNoteBuilder {
        diversifier: sapling_crypto::Diversifier,
        note: sapling_crypto::Note,
        witnessed_position: Option<Position>,
        output_index: Option<u32>,
        nullifier: Option<sapling_crypto::Nullifier>,
        spent: Option<(TxId, u32)>,
        unconfirmed_spent: Option<(TxId, u32)>,
        memo: Option<Memo>,
        is_change: bool,
        have_spending_key: bool,
    }

    #[allow(dead_code)] //TODO:  fix this gross hack that I tossed in to silence the language-analyzer false positive
    impl SaplingNoteBuilder {
        pub fn new() -> Self {
            Self::default()
        }

        // Methods to set each field
        build_method!(diversifier, sapling_crypto::Diversifier);
        build_method!(note, sapling_crypto::Note);
        build_method!(witnessed_position, Option<Position>);
        build_method!(output_index, Option<u32>);
        build_method!(nullifier, Option<sapling_crypto::Nullifier>);
        build_method!(spent, Option<(TxId, u32)>);
        build_method!(unconfirmed_spent, Option<(TxId, u32)>);
        build_method!(memo, Option<Memo>);
        pub fn set_change(mut self, is_change: bool) -> Self {
            self.is_change = is_change;
            self
        }
        build_method!(have_spending_key, bool);

        // Build method
        pub fn build(self) -> SaplingNote {
            SaplingNote::from_parts(
                self.diversifier,
                self.note,
                self.witnessed_position,
                self.nullifier,
                self.spent,
                self.unconfirmed_spent,
                self.memo,
                self.is_change,
                self.have_spending_key,
                self.output_index,
            )
        }
    }

    impl Default for SaplingNoteBuilder {
        fn default() -> Self {
            SaplingNoteBuilder {
                diversifier: sapling_crypto::Diversifier([0; 11]),
                note: crate::test_framework::mocks::mock_sapling_crypto_note(),
                witnessed_position: Some(Position::from(0)),
                output_index: Some(0),
                nullifier: Some(sapling_crypto::Nullifier::from_bytes([0; 32])),
                spent: None,
                unconfirmed_spent: None,
                memo: None,
                is_change: false,
                have_spending_key: true,
            }
        }
    }

    impl SaplingNote {
        #[allow(dead_code)]
        pub(crate) fn mock() -> Self {
            SaplingNoteBuilder::default().build()
        }
    }
}

#[cfg(test)]
#[cfg(feature = "test-features")]
pub mod tests {
    use crate::{
        test_framework::mocks::mock_txid,
        wallet::notes::{sapling::mocks::SaplingNoteBuilder, NoteInterface},
    };

    #[test]
    fn pending_spent_note_is_pending_spent() {
        let spend = Some((mock_txid(), 112358));
        let note = SaplingNoteBuilder::default()
            .unconfirmed_spent(spend)
            .build();
        assert_eq!(note.is_spent(), false);
        assert_eq!(note.is_pending_spent(), true);
        assert_eq!(note.is_spent_or_pending_spent(), true);
        assert_eq!(note.pending_spent(), &spend);
    }
}

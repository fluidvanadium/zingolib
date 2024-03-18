use shardtree::error::ShardTreeError;
use zcash_client_backend::data_api::chain::CommitmentTreeRoot;
use zcash_client_backend::data_api::WalletCommitmentTrees;

use crate::wallet::WitnessTrees;

use super::LightWallet;

impl WalletCommitmentTrees for LightWallet {
    type Error = Infallible;

    type SaplingShardStore<'a> = SapStore;

    fn with_sapling_tree_mut<F, A, E>(&mut self, _callback: F) -> Result<A, E>
    where
        for<'a> F: FnMut(
            &'a mut ShardTree<
                Self::SaplingShardStore<'a>,
                { sapling_crypto::NOTE_COMMITMENT_TREE_DEPTH },
                SAPLING_SHARD_HEIGHT,
            >,
        ) -> Result<A, E>,
        E: From<ShardTreeError<Self::Error>>,
    {
        unimplemented!();
    }

    fn put_sapling_subtree_roots(
        &mut self,
        _start_index: u64,
        _roots: &[CommitmentTreeRoot<sapling_crypto::Node>],
    ) -> Result<(), ShardTreeError<Self::Error>> {
        unimplemented!();
    }

    type OrchardShardStore<'a> = OrchStore;

    fn with_orchard_tree_mut<F, A, E>(&mut self, mut callback: F) -> Result<A, E>
    where
        for<'a> F: FnMut(
            &'a mut ShardTree<
                Self::OrchardShardStore<'a>,
                { ORCHARD_SHARD_HEIGHT * 2 },
                ORCHARD_SHARD_HEIGHT,
            >,
        ) -> Result<A, E>,
        E: From<ShardTreeError<Self::Error>>,
    {
        unimplemented!();
        // //review! ensure spend_capable wallet and break before this case
        // let op_witness_trees = &mut self.witness_trees;
        // let witness_trees: &mut WitnessTrees = op_witness_trees.as_mut().unwrap();
        // let witness_tree_orchard = &mut witness_trees.witness_tree_orchard;
        // callback(witness_tree_orchard)
    }

    fn put_orchard_subtree_roots(
        &mut self,
        _start_index: u64,
        _roots: &[CommitmentTreeRoot<orchard::tree::MerkleHashOrchard>],
    ) -> Result<(), ShardTreeError<Self::Error>> {
        unimplemented!();
    }
}

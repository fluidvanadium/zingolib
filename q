[1mdiff --git a/zingolib/src/wallet/disk/testing/tests.rs b/zingolib/src/wallet/disk/testing/tests.rs[m
[1mindex 15e90d65e..21a62d70d 100644[m
[1m--- a/zingolib/src/wallet/disk/testing/tests.rs[m
[1m+++ b/zingolib/src/wallet/disk/testing/tests.rs[m
[36m@@ -39,7 +39,13 @@[m [masync fn verify_example_wallet_regtest_aaaaaaaaaaaaaaaaaaaaaaaa_v26() {[m
     )))[m
     .await;[m
 [m
[31m-    loaded_wallet_assert(wallet, 10342837, 3).await;[m
[32m+[m[32m    loaded_wallet_assert([m
[32m+[m[32m        wallet,[m
[32m+[m[32m        crate::testvectors::seeds::CHIMNEY_BETTER_SEED.to_string(),[m
[32m+[m[32m        10342837,[m
[32m+[m[32m        3,[m
[32m+[m[32m    )[m
[32m+[m[32m    .await;[m
 }[m
 [m
 #[tokio::test][m
[36m@@ -56,7 +62,13 @@[m [masync fn verify_example_wallet_testnet_cbbhrwiilgbrababsshsmtpr_v26() {[m
     )))[m
     .await;[m
 [m
[31m-    loaded_wallet_assert(wallet, 0, 3).await;[m
[32m+[m[32m    loaded_wallet_assert([m
[32m+[m[32m        wallet,[m
[32m+[m[32m        crate::testvectors::seeds::CHIMNEY_BETTER_SEED.to_string(),[m
[32m+[m[32m        0,[m
[32m+[m[32m        3,[m
[32m+[m[32m    )[m
[32m+[m[32m    .await;[m
 }[m
 #[ignore = "test proves note has no index bug is a breaker"][m
 #[tokio::test][m
[36m@@ -66,7 +78,13 @@[m [masync fn verify_example_wallet_testnet_cbbhrwiilgbrababsshsmtpr_v27() {[m
     )))[m
     .await;[m
 [m
[31m-    loaded_wallet_assert(wallet, 10177826, 1).await;[m
[32m+[m[32m    loaded_wallet_assert([m
[32m+[m[32m        wallet,[m
[32m+[m[32m        crate::testvectors::seeds::CHIMNEY_BETTER_SEED.to_string(),[m
[32m+[m[32m        10177826,[m
[32m+[m[32m        1,[m
[32m+[m[32m    )[m
[32m+[m[32m    .await;[m
 }[m
 #[tokio::test][m
 async fn verify_example_wallet_testnet_cbbhrwiilgbrababsshsmtpr_v28() {[m
[36m@@ -85,14 +103,12 @@[m [masync fn verify_example_wallet_mainnet_vtfcorfbcbpctcfupmegmwbp_v28() {[m
 [m
 async fn loaded_wallet_assert([m
     wallet: LightWallet,[m
[32m+[m[32m    expected_seed_phrase: String,[m
     expected_balance: u64,[m
     expected_num_addresses: usize,[m
 ) {[m
     let expected_mnemonic = ([m
[31m-        zcash_primitives::zip339::Mnemonic::from_phrase([m
[31m-            crate::testvectors::seeds::CHIMNEY_BETTER_SEED.to_string(),[m
[31m-        )[m
[31m-        .unwrap(),[m
[32m+[m[32m        zcash_primitives::zip339::Mnemonic::from_phrase(expected_seed_phrase).unwrap(),[m
         0,[m
     );[m
     assert_eq!(wallet.mnemonic(), Some(&expected_mnemonic));[m

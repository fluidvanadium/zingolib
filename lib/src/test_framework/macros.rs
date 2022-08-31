#[macro_export]
macro_rules! apply_scenario {
    ($test_name:ident $numblocks:literal) => {
        concat_idents::concat_idents!(
            fn_name = scenario_, $test_name {
                #[tokio::test]
                async fn fn_name() {
                    let mut scenario = $crate::lightclient::test_server::setup_n_block_fcbl_scenario($numblocks).await;
                    $test_name(&mut scenario).await;
                    clean_shutdown(scenario.stop_transmitter, scenario.test_server_handle).await;
                }
            }
        );
    };
}

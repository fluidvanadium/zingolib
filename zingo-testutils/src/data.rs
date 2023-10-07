pub mod seeds {
    #[test]
    fn validate_seeds() {
        let abandon_art_seed = zcash_primitives::zip339::Mnemonic::from_entropy([0; 32])
            .unwrap()
            .to_string();
        assert_eq!(ABANDON_ART_SEED, abandon_art_seed);
        // TODO user get_zaddr_from_bip39seed to generate this address from that seed.
    }
    //Generate test seed
    pub const ABANDON_ART_SEED: &str =
        "abandon abandon abandon abandon abandon abandon abandon abandon \
     abandon abandon abandon abandon abandon abandon abandon abandon \
     abandon abandon abandon abandon abandon abandon abandon art";
    pub const HOSPITAL_MUSEUM_SEED: &str = "hospital museum valve antique skate museum \
     unfold vocal weird milk scale social vessel identify \
     crowd hospital control album rib bulb path oven civil tank";
}
pub const REGSAP_ADDR_FROM_ABANDONART: &str =
    "zregtestsapling1fmq2ufux3gm0v8qf7x585wj56le4wjfsqsj27zprjghntrerntggg507hxh2ydcdkn7sx8kya7p";
pub mod config_template_fillers {
    pub mod zcashd {
        use zcash_primitives::consensus::NetworkUpgrade;

        pub fn basic(
            rpcport: &str,
            regtest_network: zingoconfig::RegtestNetwork,
            extra: &str,
        ) -> String {
            let overwinter_activation_height = regtest_network
                .activation_height(NetworkUpgrade::Overwinter)
                .unwrap();
            let sapling_activation_height = regtest_network
                .activation_height(NetworkUpgrade::Sapling)
                .unwrap();
            let blossom_activation_height = regtest_network
                .activation_height(NetworkUpgrade::Blossom)
                .unwrap();
            let heartwood_activation_height = regtest_network
                .activation_height(NetworkUpgrade::Heartwood)
                .unwrap();
            let canopy_activation_height = regtest_network
                .activation_height(NetworkUpgrade::Canopy)
                .unwrap();
            let orchard_activation_height = regtest_network
                .activation_height(NetworkUpgrade::Nu5)
                .unwrap();

            format!("\
### Blockchain Configuration
regtest=1
nuparams=5ba81b19:{overwinter_activation_height} # Overwinter
nuparams=76b809bb:{sapling_activation_height} # Sapling
nuparams=2bb40e60:{blossom_activation_height} # Blossom
nuparams=f5b9230b:{heartwood_activation_height} # Heartwood
nuparams=e9ff75a6:{canopy_activation_height} # Canopy
nuparams=c2d6d0b4:{orchard_activation_height} # NU5 (Orchard)

### MetaData Storage and Retrieval
# txindex:
# https://zcash.readthedocs.io/en/latest/rtd_pages/zcash_conf_guide.html#miscellaneous-options
txindex=1
# insightexplorer:
# https://zcash.readthedocs.io/en/latest/rtd_pages/insight_explorer.html?highlight=insightexplorer#additional-getrawtransaction-fields
insightexplorer=1
experimentalfeatures=1

### RPC Server Interface Options:
# https://zcash.readthedocs.io/en/latest/rtd_pages/zcash_conf_guide.html#json-rpc-options
rpcuser=xxxxxx
rpcpassword=xxxxxx
rpcport={rpcport}
rpcallowip=127.0.0.1

# Buried config option to allow non-canonical RPC-PORT:
# https://zcash.readthedocs.io/en/latest/rtd_pages/zcash_conf_guide.html#zcash-conf-guide
listen=0

{extra}"
            )
        }
        pub fn funded(
            mineraddress: &str,
            rpcport: &str,
            regtest_network: zingoconfig::RegtestNetwork,
        ) -> String {
            basic(rpcport, regtest_network,
                &format!("\
### Zcashd Help provides documentation of the following:
mineraddress={mineraddress}
minetolocalwallet=0 # This is set to false so that we can mine to a wallet, other than the zcashd wallet."
                )
            )
        }

        #[test]
        fn funded_zcashd_conf() {
            let regtest_network = zingoconfig::RegtestNetwork::new(1, 2, 3, 4, 5, 6);
            assert_eq!(
                        funded(
                            super::super::REGSAP_ADDR_FROM_ABANDONART,
                            "1234",
                            regtest_network
                        ),
                        format!("\
### Blockchain Configuration
regtest=1
nuparams=5ba81b19:1 # Overwinter
nuparams=76b809bb:2 # Sapling
nuparams=2bb40e60:3 # Blossom
nuparams=f5b9230b:4 # Heartwood
nuparams=e9ff75a6:5 # Canopy
nuparams=c2d6d0b4:6 # NU5 (Orchard)

### MetaData Storage and Retrieval
# txindex:
# https://zcash.readthedocs.io/en/latest/rtd_pages/zcash_conf_guide.html#miscellaneous-options
txindex=1
# insightexplorer:
# https://zcash.readthedocs.io/en/latest/rtd_pages/insight_explorer.html?highlight=insightexplorer#additional-getrawtransaction-fields
insightexplorer=1
experimentalfeatures=1

### RPC Server Interface Options:
# https://zcash.readthedocs.io/en/latest/rtd_pages/zcash_conf_guide.html#json-rpc-options
rpcuser=xxxxxx
rpcpassword=xxxxxx
rpcport=1234
rpcallowip=127.0.0.1

# Buried config option to allow non-canonical RPC-PORT:
# https://zcash.readthedocs.io/en/latest/rtd_pages/zcash_conf_guide.html#zcash-conf-guide
listen=0

### Zcashd Help provides documentation of the following:
mineraddress=zregtestsapling1fmq2ufux3gm0v8qf7x585wj56le4wjfsqsj27zprjghntrerntggg507hxh2ydcdkn7sx8kya7p
minetolocalwallet=0 # This is set to false so that we can mine to a wallet, other than the zcashd wallet."
                        )
                    );
        }
    }
    pub mod lightwalletd {
        pub fn basic(rpcport: &str) -> String {
            format!(
                "\
# # Default zingo lib lightwalletd conf YAML for regtest mode # #
grpc-bind-addr: 127.0.0.1:{rpcport}
cache-size: 10
log-file: ../logs/lwd.log
log-level: 10
zcash-conf-path: ../conf/zcash.conf

# example config for TLS
#tls-cert: /secrets/lightwallted/example-only-cert.pem
#tls-key: /secrets/lightwallted/example-only-cert.key"
            )
        }

        #[test]
        fn basic_lightwalletd_conf() {
            assert_eq!(
                basic("1234"),
                format!(
                    "\
# # Default zingo lib lightwalletd conf YAML for regtest mode # #
grpc-bind-addr: 127.0.0.1:1234
cache-size: 10
log-file: ../logs/lwd.log
log-level: 10
zcash-conf-path: ../conf/zcash.conf

# example config for TLS
#tls-cert: /secrets/lightwallted/example-only-cert.pem
#tls-key: /secrets/lightwallted/example-only-cert.key"
                )
            )
        }
    }
}

#[macro_use]
extern crate rust_embed;

pub mod blaze;
pub mod commands;
pub mod compact_formats;
pub mod grpc_connector;
pub mod lightclient;
pub mod wallet;

#[cfg(feature = "embed_params")]
#[derive(RustEmbed)]
#[folder = "zcash-params/"]
pub struct SaplingParams;
use std::{
    io::{ErrorKind, Result},
    sync::{Arc, RwLock},
};
use tokio::runtime::Runtime;
use zingoconfig::{Network, ZingoConfig};

pub fn create_on_data_dir(
    server: http::Uri,
    data_dir: Option<String>,
) -> Result<(ZingoConfig, u64)> {
    use std::net::ToSocketAddrs;

    let lc = Runtime::new().unwrap().block_on(async move {
        // Test for a connection first
        format!("{}:{}", server.host().unwrap(), server.port().unwrap())
            .to_socket_addrs()?
            .next()
            .ok_or(std::io::Error::new(
                ErrorKind::ConnectionRefused,
                "Couldn't resolve server!",
            ))?;

        // Do a getinfo first, before opening the wallet
        let info = grpc_connector::GrpcConnector::get_info(server.clone())
            .await
            .map_err(|e| std::io::Error::new(ErrorKind::ConnectionRefused, e))?;

        // Create a Light Client Config
        let config = ZingoConfig {
            server: Arc::new(RwLock::new(server)),
            chain: match info.chain_name.as_str() {
                "main" => Network::Mainnet,
                "test" => Network::Testnet,
                "regtest" => Network::Regtest,
                "fakemainnet" => Network::FakeMainnet,
                _ => panic!("Unknown network"),
            },
            monitor_mempool: true,
            anchor_offset: zingoconfig::ANCHOR_OFFSET,
            data_dir,
        };

        Ok((config, info.block_height))
    });

    lc
}

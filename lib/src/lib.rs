#![forbid(unsafe_code)]
#[macro_use]
extern crate rust_embed;
mod test_framework;

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
use zingoconfig::{BlockChain, ZingoConfig};

pub fn create_zingoconf_with_datadir(
    server: http::Uri,
    data_dir: Option<String>,
) -> Result<(ZingoConfig, u64)> {
    use std::net::ToSocketAddrs;

    Runtime::new().unwrap().block_on(async move {
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
            server_uri: Arc::new(RwLock::new(server)),
            chain: match info.chain_name.as_str() {
                "main" => BlockChain::Mainnet,
                "test" => BlockChain::Testnet,
                "regtest" => BlockChain::Regtest,
                "fakemainnet" => BlockChain::FakeMainnet,
                _ => panic!("Unknown network"),
            },
            monitor_mempool: true,
            anchor_offset: zingoconfig::ANCHOR_OFFSET,
            data_dir,
        };

        Ok((config, info.block_height))
    })
}

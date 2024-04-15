//! ZingoLib
//! TODO: Add Crate Discription Here!

#![warn(missing_docs)]
#![forbid(unsafe_code)]
#[macro_use]
extern crate rust_embed;

/// TODO: Add Mod Description Here!
pub mod blaze;

pub mod commands;

pub mod data;

pub mod error;

pub mod grpc_connector;

pub mod lightclient;

// General library utilities such as parsing and conversions.
pub mod utils;

pub mod wallet;

#[cfg(feature = "test")]
pub use zingo_testvectors as testvectors;

#[cfg(feature = "test")]
pub(crate) mod test_framework;

// This line includes the generated `git_description()` function directly into this scope.
include!(concat!(env!("OUT_DIR"), "/git_description.rs"));

/// TODO: Add Doc Comment Here!
#[cfg(feature = "embed_params")]
#[derive(RustEmbed)]
#[folder = "zcash-params/"]
pub struct SaplingParams;

/// TODO: Add Doc Comment Here!
pub fn get_latest_block_height(lightwalletd_uri: http::Uri) -> std::io::Result<u64> {
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async move {
            crate::grpc_connector::get_info(lightwalletd_uri)
                .await
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::ConnectionRefused, e))
        })
        .map(|ld_info| ld_info.block_height)
}

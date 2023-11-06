use super::*;

impl LightClient {
    /// The wallet this fn associates with the lightclient is specifically derived from
    /// a spend authority.
    /// this pubfn is consumed in zingocli, zingo-mobile, and ZingoPC
    pub fn create_from_wallet_base(
        wallet_base: WalletBase,
        config: &ZingoConfig,
        birthday: u64,
        overwrite: bool,
    ) -> io::Result<Self> {
        Runtime::new().unwrap().block_on(async move {
            LightClient::create_from_wallet_base_async(wallet_base, config, birthday, overwrite)
                .await
        })
    }

    fn create_with_new_wallet(config: &ZingoConfig, height: u64) -> io::Result<Self> {
        Runtime::new().unwrap().block_on(async move {
            let l = LightClient::create_unconnected(config, WalletBase::FreshEntropy, height)?;
            l.set_wallet_initial_state(height).await;

            debug!("Created new wallet with a new seed!");
            debug!("Created LightClient to {}", &config.get_lightwalletd_uri());

            // Save
            l.save_internal_rust()
                .await
                .map_err(|s| io::Error::new(ErrorKind::PermissionDenied, s))?;

            Ok(l)
        })
    }
    pub fn do_seed_phrase_sync(&self) -> Result<AccountBackupInfo, &str> {
        Runtime::new()
            .unwrap()
            .block_on(async move { self.do_seed_phrase().await })
    }
    /// This function is the sole correct way to ask LightClient to save.
    pub fn export_save_buffer_runtime(&self) -> Result<Vec<u8>, String> {
        Runtime::new()
            .unwrap()
            .block_on(async move { self.export_save_buffer_async().await })
            .map_err(String::from)
    }

    /// Create a brand new wallet with a new seed phrase. Will fail if a wallet file
    /// already exists on disk
    pub fn new(config: &ZingoConfig, latest_block: u64) -> io::Result<Self> {
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        {
            if config.wallet_path_exists() {
                return Err(Error::new(
                    ErrorKind::AlreadyExists,
                    "Cannot create a new wallet from seed, because a wallet already exists",
                ));
            }
        }

        Self::create_with_new_wallet(config, latest_block)
    }

    /// This constructor depends on a wallet that's read from a buffer.
    /// It is used internally by read_from_disk, and directly called by
    /// zingo-mobile.
    pub fn read_wallet_from_buffer_runtime<R: Read>(
        config: &ZingoConfig,
        reader: R,
    ) -> io::Result<Self> {
        Runtime::new()
            .unwrap()
            .block_on(async move { Self::read_wallet_from_buffer_async(config, reader).await })
    }

    pub fn read_wallet_from_disk(config: &ZingoConfig) -> io::Result<Self> {
        let wallet_path = if config.wallet_path_exists() {
            config.get_wallet_path()
        } else {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!(
                    "Cannot read wallet. No file at {}",
                    config.get_wallet_path().display()
                ),
            ));
        };
        LightClient::read_wallet_from_buffer_runtime(
            config,
            BufReader::new(File::open(wallet_path)?),
        )
    }
}
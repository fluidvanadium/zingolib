use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use log::{error, info};

use zingoconfig::{Network, ZingoConfig};
use zingolib::{commands, lightclient::LightClient};

pub mod version;

#[macro_export]
macro_rules! configure_clapapp {
    ( $freshapp: expr ) => {
    $freshapp.version(VERSION)
            .arg(Arg::with_name("nosync")
                .help("By default, zingo-cli will sync the wallet at startup. Pass --nosync to prevent the automatic sync at startup.")
                .long("nosync")
                .short("n")
                .takes_value(false))
            .arg(Arg::with_name("recover")
                .long("recover")
                .help("Attempt to recover the seed from the wallet")
                .takes_value(false))
            .arg(Arg::with_name("password")
                .long("password")
                .help("When recovering seed, specify a password for the encrypted wallet")
                .takes_value(true))
            .arg(Arg::with_name("seed")
                .short("s")
                .long("seed")
                .value_name("seed_phrase")
                .help("Create a new wallet with the given 24-word seed phrase. Will fail if wallet already exists")
                .takes_value(true))
            .arg(Arg::with_name("birthday")
                .long("birthday")
                .value_name("birthday")
                .help("Specify wallet birthday when restoring from seed. This is the earlist block height where the wallet has a transaction.")
                .takes_value(true))
            .arg(Arg::with_name("server")
                .long("server")
                .value_name("server")
                .help("Lightwalletd server to connect to.")
                .takes_value(true)
                .default_value(zingoconfig::DEFAULT_SERVER)
                .takes_value(true))
            .arg(Arg::with_name("data-dir")
                .long("data-dir")
                .value_name("data-dir")
                .help("Absolute path to use as data directory")
                .takes_value(true))
            .arg(Arg::with_name("regtest")
                .long("regtest")
                .value_name("regtest")
                .help("Regtest mode")
                .takes_value(false))
            .arg(Arg::with_name("COMMAND")
                .help("Command to execute. If a command is not specified, zingo-cli will start in interactive mode.")
                .required(false)
                .index(1))
            .arg(Arg::with_name("PARAMS")
                .help("Params to execute command with. Run the 'help' command to get usage help.")
                .required(false)
                .multiple(true))
    };
}

/// This function is only tested against Linux.
pub fn report_permission_error() {
    let user = std::env::var("USER").expect("Unexpected error reading value of $USER!");
    let home = std::env::var("HOME").expect("Unexpected error reading value of $HOME!");
    let current_executable =
        std::env::current_exe().expect("Unexpected error reporting executable path!");
    eprintln!("USER: {}", user);
    eprintln!("HOME: {}", home);
    eprintln!("Executable: {}", current_executable.display());
    if home == "/" {
        eprintln!(
            "User {} must have permission to write to '{}.zcash/' .",
            user, home
        );
    } else {
        eprintln!(
            "User {} must have permission to write to '{}/.zcash/' .",
            user, home
        );
    }
}

use std::io::{ErrorKind, Result};
use tokio::runtime::Runtime;
trait ClientAsync {
    fn create_on_data_dir(
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
            let info = zingolib::grpc_connector::GrpcConnector::get_info(server.clone())
                .await
                .map_err(|e| std::io::Error::new(ErrorKind::ConnectionRefused, e))?;

            // Create a Light Client Config
            let config = ZingoConfig {
                server,
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
    fn create(server: http::Uri) -> std::io::Result<(ZingoConfig, u64)> {
        Self::create_on_data_dir(server, None)
    }
}

impl ClientAsync for ZingoConfig {}

pub fn startup(
    server: http::Uri,
    seed: Option<String>,
    birthday: u64,
    data_dir: Option<String>,
    first_sync: bool,
    print_updates: bool,
    regtest: bool,
) -> std::io::Result<(Sender<(String, Vec<String>)>, Receiver<String>)> {
    // Try to get the configuration
    let (config, latest_block_height) = ZingoConfig::create_on_data_dir(server.clone(), data_dir)?;

    // Diagnostic check for regtest flag and network in config, panic if mis-matched.
    if regtest && config.chain == Network::Regtest {
        println!("regtest detected and network set correctly!");
    } else if regtest && config.chain != Network::Regtest {
        println!("Regtest flag detected, but unexpected network set! Exiting.");
        panic!("Regtest Network Problem");
    } else if config.chain == Network::Regtest {
        println!("WARNING! regtest network in use but no regtest flag recognized!");
    }

    let lightclient = match seed {
        Some(phrase) => Arc::new(LightClient::new_from_phrase(
            phrase, &config, birthday, false,
        )?),
        None => {
            if config.wallet_exists() {
                Arc::new(LightClient::read_from_disk(&config)?)
            } else {
                println!("Creating a new wallet");
                // Create a wallet with height - 100, to protect against reorgs
                Arc::new(LightClient::new(
                    &config,
                    latest_block_height.saturating_sub(100),
                )?)
            }
        }
    };

    // Initialize logging
    lightclient.init_logging()?;

    // Print startup Messages
    info!(""); // Blank line
    info!("Starting Zingo-CLI");
    info!("Light Client config {:?}", config);

    if print_updates {
        println!("Lightclient connecting to {}", config.server);
    }

    // At startup, run a sync.
    if first_sync {
        let update = commands::do_user_command("sync", &vec![], lightclient.as_ref());
        if print_updates {
            println!("{}", update);
        }
    }

    // Start the command loop
    let (command_transmitter, resp_receiver) = command_loop(lightclient.clone());

    Ok((command_transmitter, resp_receiver))
}

pub fn start_interactive(
    command_transmitter: Sender<(String, Vec<String>)>,
    resp_receiver: Receiver<String>,
) {
    // `()` can be used when no completer is required
    let mut rl = rustyline::Editor::<()>::new();

    println!("Ready!");

    let send_command = |cmd: String, args: Vec<String>| -> String {
        command_transmitter.send((cmd.clone(), args)).unwrap();
        match resp_receiver.recv() {
            Ok(s) => s,
            Err(e) => {
                let e = format!("Error executing command {}: {}", cmd, e);
                eprintln!("{}", e);
                error!("{}", e);
                return "".to_string();
            }
        }
    };

    let info = send_command("info".to_string(), vec![]);
    let chain_name = json::parse(&info).unwrap()["chain_name"]
        .as_str()
        .unwrap()
        .to_string();

    loop {
        // Read the height first
        let height = json::parse(&send_command(
            "height".to_string(),
            vec!["false".to_string()],
        ))
        .unwrap()["height"]
            .as_i64()
            .unwrap();

        let readline = rl.readline(&format!(
            "({}) Block:{} (type 'help') >> ",
            chain_name, height
        ));
        match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_str());
                // Parse command line arguments
                let mut cmd_args = match shellwords::split(&line) {
                    Ok(args) => args,
                    Err(_) => {
                        println!("Mismatched Quotes");
                        continue;
                    }
                };

                if cmd_args.is_empty() {
                    continue;
                }

                let cmd = cmd_args.remove(0);
                let args: Vec<String> = cmd_args;

                println!("{}", send_command(cmd, args));

                // Special check for Quit command.
                if line == "quit" {
                    break;
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                println!("CTRL-C");
                info!("CTRL-C");
                println!("{}", send_command("save".to_string(), vec![]));
                break;
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                println!("CTRL-D");
                info!("CTRL-D");
                println!("{}", send_command("save".to_string(), vec![]));
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }
}

pub fn command_loop(
    lightclient: Arc<LightClient>,
) -> (Sender<(String, Vec<String>)>, Receiver<String>) {
    let (command_transmitter, command_receiver) = channel::<(String, Vec<String>)>();
    let (resp_transmitter, resp_receiver) = channel::<String>();

    let lc = lightclient.clone();
    std::thread::spawn(move || {
        LightClient::start_mempool_monitor(lc.clone());

        loop {
            if let Ok((cmd, args)) = command_receiver.recv() {
                let args = args.iter().map(|s| s.as_ref()).collect();

                let cmd_response = commands::do_user_command(&cmd, &args, lc.as_ref());
                resp_transmitter.send(cmd_response).unwrap();

                if cmd == "quit" {
                    info!("Quit");
                    break;
                }
            } else {
                break;
            }
        }
    });

    (command_transmitter, resp_receiver)
}

pub fn attempt_recover_seed(_password: Option<String>) {
    // Create a Light Client Config in an attempt to recover the file.
    let _config = ZingoConfig {
        server: "0.0.0.0:0".parse().unwrap(),
        chain: zingoconfig::Network::Mainnet,
        monitor_mempool: false,
        anchor_offset: [0u32; 5],
        data_dir: None,
    };
}

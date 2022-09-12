use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, RwLock};

use log::{error, info};

use clap::{self, Arg};
use zingoconfig::{Network, ZingoConfig};
use zingolib::{commands, create_on_data_dir, lightclient::LightClient};

pub mod regtest;
pub mod version;

pub fn configure_app() -> clap::App<'static> {
    clap::App::new("Zingo CLI").version(version::VERSION)
            .arg(Arg::with_name("nosync")
                .help("By default, zingo-cli will sync the wallet at startup. Pass --nosync to prevent the automatic sync at startup.")
                .long("nosync")
                .short('n')
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
                .short('s')
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
            .arg(Arg::with_name("no-clean")
                .long("no-clean")
                .value_name("no-clean")
                .help("Don't clean regtest state before running. Regtest mode only")
                .takes_value(false))
            .arg(Arg::with_name("COMMAND")
                .help("Command to execute. If a command is not specified, zingo-cli will start in interactive mode.")
                .required(false)
                .index(1))
            .arg(Arg::with_name("PARAMS")
                .help("Params to execute command with. Run the 'help' command to get usage help.")
                .required(false)
                .multiple(true)
                .index(2))
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
    let (config, latest_block_height) = create_on_data_dir(server.clone(), data_dir)?;

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
        println!(
            "Lightclient connecting to {}",
            config.server.read().unwrap()
        );
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
        server: Arc::new(RwLock::new("0.0.0.0:0".parse().unwrap())),
        chain: zingoconfig::Network::Mainnet,
        monitor_mempool: false,
        max_transaction_size: Arc::new(RwLock::new(zingoconfig::MAX_TRANSACTION_SIZE_DEFAULT)),
        anchor_offset: [0u32; 5],
        data_dir: None,
    };
}

pub struct CLIRunner {
    params: Vec<String>,
    password: Option<String>,
    recover: bool,
    server: http::Uri,
    seed: Option<String>,
    birthday: u64,
    maybe_data_dir: Option<String>,
    sync: bool,
    command: Option<String>,
    regtest_mode_enabled: bool,
}
use commands::ShortCircuitedCommand;
fn short_circuit_on_help(params: Vec<String>) {
    for h in commands::HelpCommand::exec_without_lc(params).lines() {
        println!("{}", h);
    }
    std::process::exit(0x0100);
}
impl CLIRunner {
    fn new() -> Self {
        let configured_app = configure_app();
        let matches = configured_app.get_matches();
        let command = matches.value_of("COMMAND");
        // Begin short_circuit section
        let params: Vec<String> = matches
            .values_of("PARAMS")
            .map(|v| v.collect())
            .or(Some(vec![]))
            .unwrap()
            .iter()
            .map(|s| s.to_string())
            .collect();
        let command = if let Some(refstr) = command {
            if refstr == "help" {
                short_circuit_on_help(params.clone());
            }
            Some(refstr.to_string())
        } else {
            None
        };
        let recover = matches.is_present("recover");
        let seed = matches.value_of("seed").map(|s| s.to_string());
        let maybe_birthday = matches.value_of("birthday");
        if seed.is_some() && maybe_birthday.is_none() {
            eprintln!("ERROR!");
            eprintln!(
            "Please specify the wallet birthday (eg. '--birthday 600000') to restore from seed."
        );
            panic!("This should be the block height where the wallet was created. If you don't remember the block height, you can pass '--birthday 0' to scan from the start of the blockchain.");
        }
        let birthday = match maybe_birthday.unwrap_or("0").parse::<u64>() {
            Ok(b) => b,
            Err(e) => {
                panic!(
                    "Couldn't parse birthday. This should be a block number. Error={}",
                    e
                );
            }
        };

        let maybe_server = matches.value_of("server").map(|s| s.to_string());

        let maybe_data_dir = matches.value_of("data-dir").map(|s| s.to_string());

        let regtest_mode_enabled = matches.is_present("regtest");
        let clean_regtest_data = !matches.is_present("no-clean");
        let server = if regtest_mode_enabled {
            (regtest::RegtestManager::new()).launch(clean_regtest_data);
            ZingoConfig::get_server_or_default(Some("http://127.0.0.1".to_string()))
            // do the regtest
        } else {
            ZingoConfig::get_server_or_default(maybe_server)
        };

        // Test to make sure the server has all of scheme, host and port
        if server.scheme_str().is_none() || server.host().is_none() || server.port().is_none() {
            panic!(
            "Please provide the --server parameter as [scheme]://[host]:[port].\nYou provided: {}",
            server
        );
        }

        let sync = !matches.is_present("nosync");
        let password = matches.value_of("password").map(|s| s.to_string());
        Self {
            params,
            password,
            recover,
            server,
            seed,
            birthday,
            maybe_data_dir,
            sync,
            command,
            regtest_mode_enabled,
        }
    }
    fn check_recover(&self) {
        if self.recover {
            // Create a Light Client Config in an attempt to recover the file.
            attempt_recover_seed(self.password.clone());
            std::process::exit(0x0100);
        }
    }
    fn start_cli_service(&self) -> (Sender<(String, Vec<String>)>, Receiver<String>) {
        let startup_chan = startup(
            self.server.clone(),
            self.seed.clone(),
            self.birthday,
            self.maybe_data_dir.clone(),
            self.sync,
            self.command.is_none(),
            self.regtest_mode_enabled,
        );
        match startup_chan {
            Ok(c) => c,
            Err(e) => {
                let emsg = format!("Error during startup:{}\nIf you repeatedly run into this issue, you might have to restore your wallet from your seed phrase.", e);
                eprintln!("{}", emsg);
                error!("{}", emsg);
                if cfg!(target_os = "unix") {
                    match e.raw_os_error() {
                        Some(13) => report_permission_error(),
                        _ => {}
                    }
                };
                panic!();
            }
        }
    }
    fn dispatch_command_or_start_interactive(&self) {
        let (command_transmitter, resp_receiver) = self.start_cli_service();
        if self.command.is_none() {
            start_interactive(command_transmitter, resp_receiver);
        } else {
            command_transmitter
                .send((
                    self.command.clone().unwrap().to_string(),
                    self.params
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<String>>(),
                ))
                .unwrap();

            match resp_receiver.recv() {
                Ok(s) => println!("{}", s),
                Err(e) => {
                    let e = format!(
                        "Error executing command {}: {}",
                        self.command.clone().unwrap(),
                        e
                    );
                    eprintln!("{}", e);
                    error!("{}", e);
                }
            }

            // Save before exit
            command_transmitter
                .send(("save".to_string(), vec![]))
                .unwrap();
            resp_receiver.recv().unwrap();
        }
    }
    pub fn run_cli() {
        let cli_runner = CLIRunner::new();
        cli_runner.check_recover();
        cli_runner.dispatch_command_or_start_interactive();
    }
}

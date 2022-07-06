use log::error;
use zingo_cli::{
    attempt_recover_seed, configure_clapapp, report_permission_error, start_interactive, startup,
    version::VERSION,
};
use zingoconfig::ZingoConfig;

pub fn main() {
    // Get command line arguments
    use clap::{App, Arg};
    let fresh_app = App::new("Zingo CLI");
    let configured_app = configure_clapapp!(fresh_app);
    let matches = configured_app.get_matches();

    if matches.is_present("recover") {
        // Create a Light Client Config in an attempt to recover the file.
        attempt_recover_seed(matches.value_of("password").map(|s| s.to_string()));
        return;
    }

    let command = matches.value_of("COMMAND");
    let params = matches
        .values_of("PARAMS")
        .map(|v| v.collect())
        .or(Some(vec![]))
        .unwrap();

    let maybe_server = matches.value_of("server").map(|s| s.to_string());

    let maybe_data_dir = matches.value_of("data-dir").map(|s| s.to_string());

    let seed = matches.value_of("seed").map(|s| s.to_string());
    let maybe_birthday = matches.value_of("birthday");

    if seed.is_some() && maybe_birthday.is_none() {
        eprintln!("ERROR!");
        eprintln!(
            "Please specify the wallet birthday (eg. '--birthday 600000') to restore from seed."
        );
        eprintln!("This should be the block height where the wallet was created. If you don't remember the block height, you can pass '--birthday 0' to scan from the start of the blockchain.");
        return;
    }

    let birthday = match maybe_birthday.unwrap_or("0").parse::<u64>() {
        Ok(b) => b,
        Err(e) => {
            eprintln!(
                "Couldn't parse birthday. This should be a block number. Error={}",
                e
            );
            return;
        }
    };

    let regtest = matches.is_present("regtest");
    if regtest {
        use std::ffi::OsString;
        use std::fs::File;
        use std::io;
        use std::path::PathBuf;
        use std::process::{Command, Stdio};
        use std::{thread, time};

        // confirm we are in a git directory, get info on it
        let revparse_raw = Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .output()
            .expect("no git!? time to quit.");

        let revparse = std::str::from_utf8(&revparse_raw.stdout).expect("revparse error");

        let ident_raw = Command::new("git")
            .args(["var", "GIT_AUTHOR_IDENT"])
            .output()
            .expect("problem ident. cannot invent!");

        let ident = std::str::from_utf8(&ident_raw.stdout).expect("ident error");
        // stand in for zingolabs.
        if ident.starts_with("dannasessha <sessha@zingolabs.com>") {
            // proceed
        } else {
            panic!("Zingo-cli's regtest mode must be run within its git tree");
        }

        // Cross-platform OsString
        let mut worktree_home = OsString::new();
        worktree_home.push(revparse.trim_end());

        // convert this back into a path for windows compatibile dir building
        let bin_location: PathBuf = [
            worktree_home.clone(),
            OsString::from("regtest"),
            OsString::from("bin"),
        ]
        .iter()
        .collect();

        let zcash_confs: PathBuf = [
            worktree_home.clone(),
            OsString::from("regtest"),
            OsString::from("conf"),
            OsString::from(""),
        ]
        .iter()
        .collect();

        let mut flagged_zcashd_conf: String = "--conf=".to_string();
        flagged_zcashd_conf.push_str(zcash_confs.to_str().expect("error making zcash_datadir"));
        flagged_zcashd_conf.push_str("zcash.conf");

        // TODO could make this less repetitive to lwd datadir
        let zcash_datadir: PathBuf = [
            worktree_home.clone(),
            OsString::from("regtest"),
            OsString::from("datadir"),
            OsString::from("zcash"),
            OsString::from(""),
        ]
        .iter()
        .collect();

        let mut flagged_datadir: String = "--datadir=".to_string();
        flagged_datadir.push_str(zcash_datadir.to_str().expect("error making zcash_datadir"));

        // currently not used.
        /*
                let zcash_logs: PathBuf = [
                    worktree_home.clone(),
                    OsString::from("regtest"),
                    OsString::from("logs"),
                ]
                .iter()
                .collect();
        */

        let mut zcashd_bin = bin_location.to_owned();
        zcashd_bin.push("zcashd");

        // TODO from zingolib as an anchor for our directory context
        // TODO reorg code, look for all needed bins ASAP
        // check for file. This might be superfluous considering
        // .expect() attached to the call, below?
        if !std::path::Path::is_file(zcashd_bin.as_path()) {
            panic!("can't find zcashd bin! exiting.");
        }
        println!("zcashd datadir: {}", &flagged_datadir);
        println!("zcashd conf file: {}", &flagged_zcashd_conf);
        let zcashd_command = Command::new(zcashd_bin)
            .args([
                "--printtoconsole",
                &flagged_zcashd_conf,
                &flagged_datadir,
                // Right now I can't get zcashd to write to debug.log with this flag
                //"-debuglogfile=.../zingolib/regtest/logs/debug.log",
                //debug=1 will at least print to stdout
                "-debug=1",
            ])
            // piping stdout off...
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to start zcashd");

        // ... to ... nowhere for now.
        //let mut zcashd_logfile =
        //File::create("/zingolib/regtest/logs/ping.log").unwrap();
        // TODO the next line is halting the process... spawn a thread for stdout
        //io::copy(&mut zcashd_command.stdout.unwrap(), &mut zcashd_logfile).unwrap();

        println!("zcashd is starting in regtest mode, please standby about 10 seconds...");
        // wait 10 seconds for zcashd to fire up
        // very generous, plan to tune down
        let ten_seconds = time::Duration::from_millis(10_000);
        thread::sleep(ten_seconds);

        // TODO this process does not shut down when rust client shuts down!
        // Needs a cleanup function, or something.
        println!("zcashd start section completed, zcashd should be running.");
        println!("Standby, lightwalletd is about to start. This should only take a moment.");

        let mut lwd_bin = bin_location.to_owned();
        lwd_bin.push("lightwalletd");

        let lwd_confs: PathBuf = [
            worktree_home.clone(),
            OsString::from("regtest"),
            OsString::from("conf"),
            OsString::from(""),
        ]
        .iter()
        .collect();

        let mut unflagged_lwd_conf: String = String::new();
        unflagged_lwd_conf.push_str(lwd_confs.to_str().expect("trouble making flagged_lwd_conf"));
        unflagged_lwd_conf.push_str("lightwalletdconf.yml");

        // for lwd config
        let mut unflagged_zcashd_conf: String = String::new();
        unflagged_zcashd_conf.push_str(
            zcash_confs
                .to_str()
                .expect("error making unflagged zcash conf"),
        );
        unflagged_zcashd_conf.push_str("zcash.conf");

        let lwd_datadir: PathBuf = [
            worktree_home.clone(),
            OsString::from("regtest"),
            OsString::from("datadir"),
            OsString::from("lightwalletd"),
            OsString::from(""),
        ]
        .iter()
        .collect();

        let mut unflagged_lwd_datadir: String = String::new();
        unflagged_lwd_datadir.push_str(lwd_datadir.to_str().expect("error making lwd_datadir"));

        let lwd_logs: PathBuf = [
            worktree_home.clone(),
            OsString::from("regtest"),
            OsString::from("logs"),
            OsString::from(""),
        ]
        .iter()
        .collect();
        let mut unflagged_lwd_log: String = String::new();
        unflagged_lwd_log.push_str(lwd_logs.to_str().expect("error making lwd_datadir"));
        unflagged_lwd_log.push_str("lwd.log");

        let lwd_command = Command::new(lwd_bin)
            .args([
                "--no-tls-very-insecure",
                "--zcash-conf-path",
                &unflagged_zcashd_conf,
                "--config",
                &unflagged_lwd_conf,
                "--data-dir",
                &unflagged_lwd_datadir,
                "--log-file",
                &unflagged_lwd_log,
            ])
            // this will print stdout of lwd process' output also to the zingo-cli stdout
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("failed to start lwd");

        // this client's stdout is less verbose so logging it may not be needed along with the working lwd.log file.
        //let mut lwd_stdout_logfile =
        //File::create("/zingolib/regtest/logs/lwd-ping.log").unwrap();
        // the next line is a halting process..
        //io::copy(&mut lwd_command.stdout.unwrap(), &mut lwd_stdout_logfile).unwrap();

        // wait 5 seconds for lwd to fire up
        // very generous, plan to tune down
        let five_seconds = time::Duration::from_millis(5_000);
        thread::sleep(five_seconds);

        // this process does not shut down when rust client shuts down!
        // TODO Needs a cleanup function, or something.
        println!("lwd start section completed, lightwalletd should be running!");
        println!("Standby, Zingo-cli should be running in regtest mode momentarily...");
    }

    let server = ZingoConfig::get_server_or_default(maybe_server);

    // Test to make sure the server has all of scheme, host and port
    if server.scheme_str().is_none() || server.host().is_none() || server.port().is_none() {
        eprintln!(
            "Please provide the --server parameter as [scheme]://[host]:[port].\nYou provided: {}",
            server
        );
        return;
    }

    let nosync = matches.is_present("nosync");

    let startup_chan = startup(
        server,
        seed,
        birthday,
        maybe_data_dir,
        !nosync,
        command.is_none(),
        regtest,
    );
    let (command_transmitter, resp_receiver) = match startup_chan {
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
            return;
        }
    };

    if command.is_none() {
        start_interactive(command_transmitter, resp_receiver);
    } else {
        command_transmitter
            .send((
                command.unwrap().to_string(),
                params
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>(),
            ))
            .unwrap();

        match resp_receiver.recv() {
            Ok(s) => println!("{}", s),
            Err(e) => {
                let e = format!("Error executing command {}: {}", command.unwrap(), e);
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

#[cfg(feature = "nigiri")]
#[allow(dead_code)]
pub mod nigiri {
    use bdk::blockchain::{ElectrumBlockchain, GetHeight};
    use bdk::electrum_client::Client;
    use log::debug;
    use simplelog::SimpleLogger;
    use std::process::{Command, Output};
    use std::sync::Once;
    use std::thread::sleep;
    use std::time::Duration;

    static INIT_LOGGER_ONCE: Once = Once::new();

    pub fn start() {
        INIT_LOGGER_ONCE.call_once(|| {
            SimpleLogger::init(simplelog::LevelFilter::Debug, simplelog::Config::default())
                .unwrap();
        });

        // Reset Nigiri state to start on a blank slate
        stop();

        start_nigiri();
    }

    pub fn stop() {
        debug!("NIGIRI stopping ...");
        exec(&["nigiri", "stop", "--delete"]);
    }

    pub fn pause() {
        debug!("NIGIRI pausing (stopping without resetting)...");
        exec(&["nigiri", "stop"]);
    }

    pub fn resume() {
        start_nigiri();
    }

    fn start_nigiri() {
        debug!("NIGIRI starting ...");
        exec(&["nigiri", "start", "--ci"]);
        wait_for_electrum();
    }

    fn wait_for_electrum() {
        let client = Client::new("localhost:50000").unwrap();
        let blockchain = ElectrumBlockchain::from(client);

        let mut i = 0u8;
        while let Err(e) = blockchain.get_height() {
            if i == 15 {
                panic!("Failed to start NIGIRI: {}", e);
            }
            i += 1;
            sleep(Duration::from_secs(1));
        }
    }

    pub fn mine_blocks(block_amount: u32) -> Result<(), String> {
        let cmd = &["nigiri", "rpc", "-generate", &block_amount.to_string()];

        let output = exec(cmd);
        if !output.status.success() {
            return Err(produce_cmd_err_msg(cmd, output));
        }
        Ok(())
    }

    pub fn fund_address(amount_btc: f32, address: &str) -> Result<(), String> {
        let cmd = &["nigiri", "faucet", &address, &amount_btc.to_string()];

        let output = exec(cmd);
        if !output.status.success() {
            return Err(produce_cmd_err_msg(cmd, output));
        }
        Ok(())
    }

    pub fn exec(params: &[&str]) -> Output {
        exec_in_dir(params, ".")
    }

    fn exec_in_dir(params: &[&str], dir: &str) -> Output {
        let (command, args) = params.split_first().expect("At least one param is needed");
        Command::new(command)
            .current_dir(dir)
            .args(args)
            .output()
            .expect("Failed to run command")
    }

    fn produce_cmd_err_msg(cmd: &[&str], output: Output) -> String {
        format!(
            "Command `{}` failed.\nStderr: {}Stdout: {}",
            cmd.join(" "),
            String::from_utf8(output.stderr).unwrap(),
            String::from_utf8(output.stdout).unwrap(),
        )
    }

    #[macro_export]
    macro_rules! try_cmd_repeatedly {
        ($func:path, $retry_times:expr, $interval:expr, $($arg:expr),*) => {{
            let mut retry_times = $retry_times;

            while let Err(e) = $func($($arg),*) {
                retry_times -= 1;

                if retry_times == 0 {
                    panic!("Failed to execute {} after {} tries: {}", stringify!($func), $retry_times, e);
                }
                sleep($interval);
            }
        }};
    }
}

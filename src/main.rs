use clap::Parser;
use confy::ConfyError;
use thiserror::Error;
use crate::api::{load_e2e_config};

mod api;

fn main() -> Result<(), E2EError>{
    let args = CliArgs::parse();
    let cfg = load_e2e_config(&args.config_path)?;

    println!("E2E config: {:?}", cfg);
    Ok(())
//     load config and testspec
//         number of messages
//         number of clients
//     get latest log entry
//     generate greetings
//     send greetings
//     verify all greetings are stored and accessible via API checks
}

/// Runs e2e test for greeting-solution.
#[derive(Parser)]
#[command(version, about, long_about = None)]
pub(crate) struct CliArgs {
    /// Path to configfile. If missing, a template file with default values is created.
    #[arg(short = 'c', long = "config")]
    pub config_path: String,
}


#[derive(Error, Debug)]
enum E2EError{
    #[error("E2E config error: {0}")]
    ConfigError(#[from] ConfyError),
}

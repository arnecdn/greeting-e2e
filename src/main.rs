use std::{fs, io};
use std::path::Path;
use clap::Parser;
use confy::ConfyError;
use serde::{Deserialize, Serialize};
use crate::api::{load_e22_config, CliArgs, E2ETestConfig};

mod api;


fn main() -> Result<(), E2EError>{
    let args = CliArgs::parse();
    let cfg = load_e22_config(&args.config_path)?;

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


#[derive(Debug)]
struct E2EError{
    message:String
}

impl From<ConfyError> for E2EError {
    fn from(value: ConfyError) -> Self {
        E2EError{message:value.to_string()}
    }
}
mod commands;

use crate::commands::beta::BetaNamespace;
use anyhow::Result;
use commands::IonCliCommand;
use ion_rs::IonError;
use std::io::ErrorKind;

use crate::commands::dump::DumpCommand;

fn main() -> Result<()> {
    let root_command = RootCommand;
    let args = root_command.clap_command().get_matches();
    let mut command_path: Vec<String> = vec![root_command.name().to_owned()];

    if let Err(e) = root_command.run(&mut command_path, &args) {
        match e.downcast_ref::<IonError>() {
            // If `ion-cli` is being invoked as part of a pipeline we want to allow the pipeline to
            // to shut off without printing an error to stderr
            Some(IonError::Io(error)) if error.source().kind() == ErrorKind::BrokenPipe => {
                return Ok(())
            }
            _ => return Err(e),
        }
    };

    Ok(())
}

pub struct RootCommand;

impl IonCliCommand for RootCommand {
    fn name(&self) -> &'static str {
        "ion"
    }

    fn about(&self) -> &'static str {
        "A collection of tools for working with Ion data."
    }

    fn subcommands(&self) -> Vec<Box<dyn IonCliCommand>> {
        vec![Box::new(BetaNamespace), Box::new(DumpCommand)]
    }
}

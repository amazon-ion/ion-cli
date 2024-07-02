mod auto_decompress;
mod commands;
mod file_writer;
mod input;
mod output;
mod transcribe;

use anyhow::Result;
use commands::{IonCliCommand, IonCliNamespace};
use ion_rs::IonError;
use std::io::ErrorKind;

use crate::commands::cat::CatCommand;
use crate::commands::count::CountCommand;
use crate::commands::from::FromNamespace;
#[cfg(feature = "experimental-code-gen")]
use crate::commands::generate::GenerateCommand;
use crate::commands::head::HeadCommand;
use crate::commands::inspect::InspectCommand;
use crate::commands::primitive::PrimitiveCommand;
use crate::commands::schema::SchemaNamespace;
use crate::commands::symtab::SymtabNamespace;
use crate::commands::to::ToNamespace;

fn main() -> Result<()> {
    let root_command = RootCommand;
    let args = root_command.clap_command().get_matches();
    let mut command_path: Vec<String> = vec![IonCliNamespace::name(&root_command).to_owned()];

    if let Err(e) = root_command.run(&mut command_path, &args) {
        match e.downcast_ref::<IonError>() {
            // If `ion-cli` is being invoked as part of a pipeline we want to allow the pipeline
            // to shut off without printing an error to STDERR.
            Some(IonError::Io(error)) if error.source().kind() == ErrorKind::BrokenPipe => {
                return Ok(());
            }
            _ => return Err(e),
        }
    };

    Ok(())
}

pub struct RootCommand;

impl IonCliNamespace for RootCommand {
    fn name(&self) -> &'static str {
        "ion"
    }

    fn about(&self) -> &'static str {
        "A collection of tools for working with Ion data."
    }

    fn subcommands(&self) -> Vec<Box<dyn IonCliCommand>> {
        vec![
            Box::new(CatCommand),
            Box::new(CountCommand),
            Box::new(FromNamespace),
            #[cfg(feature = "experimental-code-gen")]
            Box::new(GenerateCommand),
            Box::new(HeadCommand),
            Box::new(InspectCommand),
            Box::new(PrimitiveCommand),
            Box::new(SchemaNamespace),
            Box::new(SymtabNamespace),
            Box::new(ToNamespace),
        ]
    }
}

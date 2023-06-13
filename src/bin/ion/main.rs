mod commands;


use anyhow::{anyhow, Result};
use clap::{ArgMatches, Command as ClapCommand, crate_authors, crate_version};
use crate::commands::beta::BetaNamespace;

use crate::commands::dump::DumpCommand;

fn main() -> Result<()> {
    let root_command = RootCommand;
    let args = root_command.clap_command().get_matches();
    let mut command_path: Vec<String> = vec![root_command.name().to_owned()];
    root_command.run(&mut command_path, &args)
}

pub type CommandRunner = fn(&str, &ArgMatches) -> Result<()>;

pub trait IonCliCommand {
    fn name(&self) -> &'static str;

    fn about(&self) -> &'static str;

    fn clap_command(&self) -> ClapCommand {
        let clap_subcommands: Vec<_> = self.subcommands().iter().map(|s| s.clap_command()).collect();
        ClapCommand::new(self.name())
            .about(self.about())
            .version(crate_version!())
            .author(crate_authors!())
            .subcommand_required(true)
            .subcommands(clap_subcommands)
    }

    fn subcommands(&self) -> Vec<Box<dyn IonCliCommand>> {
        Vec::new()
    }

    fn get_subcommand(&self, subcommand_name: &str) -> Option<Box<dyn IonCliCommand>> {
        let mut subcommands = self.subcommands();
        if let Some(index) = subcommands.iter().position(|s| s.name() == subcommand_name) {
            Some(subcommands.swap_remove(index))
        } else {
            None
        }
    }

    // The default implementation assumes this command is a namespace (i.e. a group of subcommands).
    // It looks for a subcommand in the arguments, then looks up and runs that subcommand.
    fn run(&self, command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        let (subcommand_name, subcommand_args) = args.subcommand()
            .ok_or_else(|| anyhow!("Command '{}' expects a subcommand.", self.name()))?;

        let subcommand = self.get_subcommand(subcommand_name)
            .ok_or_else(|| anyhow!("'{}' subcommand '{}' was not recognized.", self.name(), subcommand_name))?;

        command_path.push(subcommand_name.to_owned());
        subcommand.run(command_path, subcommand_args)
    }
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
        vec![
            Box::new(BetaNamespace),
            Box::new(DumpCommand),
        ]
    }
}

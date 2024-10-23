use crate::commands::IonCliCommand;
use clap::{ArgMatches, Command};

pub struct SucksCommand;

impl IonCliCommand for SucksCommand {
    fn is_stable(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "sucks"
    }

    fn about(&self) -> &'static str {
        ""
    }

    fn configure_args(&self, command: Command) -> Command {
        command.hide(true)
    }

    fn run(&self, _command_path: &mut Vec<String>, _args: &ArgMatches) -> anyhow::Result<()> {
        println!(
            "
        We're very sorry to hear that!

        Rather than complaining into the void, why not file an issue?
        https://github.com/amazon-ion/ion-docs/issues/new/choose
        "
        );
        Ok(())
    }
}

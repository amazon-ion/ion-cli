use anyhow::anyhow;
use clap::{crate_authors, crate_version, ArgMatches, Command as ClapCommand};
pub mod beta;
pub mod dump;

pub trait IonCliCommand {
    fn name(&self) -> &'static str;

    fn about(&self) -> &'static str;

    fn configure_args(&self, _command: &mut ClapCommand) {
        // Does nothing by default
    }

    fn clap_command(&self) -> ClapCommand {
        let clap_subcommands: Vec<_> = self
            .subcommands()
            .iter()
            .map(|s| s.clap_command())
            .collect();
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
    fn run(&self, command_path: &mut Vec<String>, args: &ArgMatches) -> anyhow::Result<()> {
        let (subcommand_name, subcommand_args) = args
            .subcommand()
            .ok_or_else(|| anyhow!("Command '{}' expects a subcommand.", self.name()))?;

        let subcommand = self.get_subcommand(subcommand_name).ok_or_else(|| {
            anyhow!(
                "'{}' subcommand '{}' was not recognized.",
                self.name(),
                subcommand_name
            )
        })?;

        command_path.push(subcommand_name.to_owned());
        subcommand.run(command_path, subcommand_args)
    }
}

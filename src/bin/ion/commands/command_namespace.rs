use crate::commands::{IonCliCommand, WithIonCliArgument, UNSTABLE_FLAG};
use clap::{ArgMatches, Command as ClapCommand};
use std::process;

/// A trait that handles the implementation of [IonCliCommand] for command namespaces.
pub trait IonCliNamespace {
    fn name(&self) -> &'static str;
    fn about(&self) -> &'static str;
    fn subcommands(&self) -> Vec<Box<dyn IonCliCommand>>;
}

impl<T: IonCliNamespace> IonCliCommand for T {
    // Namespaces can't be used on their own, so we'll pretend that they are all stable and
    // let the leaf commands handle stability.
    fn is_stable(&self) -> bool {
        true
    }

    // Namespaces can't be used on their own, so we'll pretend that they are all plumbing and
    // let the leaf commands handle plumbing vs porcelain.
    fn is_porcelain(&self) -> bool {
        false
    }

    fn name(&self) -> &'static str {
        IonCliNamespace::name(self)
    }

    fn about(&self) -> &'static str {
        IonCliNamespace::about(self)
    }

    fn configure_args(&self, command: ClapCommand) -> ClapCommand {
        // Create a `ClapCommand` representing each of this command's subcommands.
        let clap_subcommands: Vec<_> = self
            .subcommands()
            .iter()
            .map(|s| s.clap_command())
            .collect();

        let mut command = command
            .subcommand_required(true)
            .subcommands(clap_subcommands);

        // If there are subcommands, add them to the configuration and set 'subcommand_required'.
        let has_unstable_subcommand = self.subcommands().iter().any(|sc| !sc.is_stable());
        if has_unstable_subcommand {
            command = command.show_unstable_flag();
        }
        command
    }

    fn run(&self, command_path: &mut Vec<String>, args: &ArgMatches) -> anyhow::Result<()> {
        // Safe to unwrap because if this is a namespace are subcommands, then clap has already
        // ensured that a known subcommand is present in args.
        let (subcommand_name, subcommand_args) = args.subcommand().unwrap();
        let subcommands = self.subcommands();
        let subcommand: &dyn IonCliCommand = subcommands
            .iter()
            .find(|sc| sc.name() == subcommand_name)
            .unwrap()
            .as_ref();

        match (subcommand.is_stable(), args.get_flag(UNSTABLE_FLAG)) {
            // Warn if using an unnecessary `-X`
            (true, true) => eprintln!(
                "'{}' is stable and does not require opt-in",
                subcommand_name
            ),
            // Error if missing a required `-X`
            (false, false) => {
                eprintln!(
                    "'{}' is unstable and requires explicit opt-in",
                    subcommand_name
                );
                process::exit(1)
            }
            _ => {}
        }

        command_path.push(subcommand_name.to_owned());
        subcommand.run(command_path, subcommand_args)
    }
}

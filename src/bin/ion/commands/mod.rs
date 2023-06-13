use anyhow::anyhow;
use clap::{crate_authors, crate_version, Arg, ArgAction, ArgMatches, Command as ClapCommand};
pub mod beta;
pub mod dump;

pub trait IonCliCommand {
    fn name(&self) -> &'static str;

    fn about(&self) -> &'static str;

    fn clap_command(&self) -> ClapCommand {
        let clap_subcommands: Vec<_> = self
            .subcommands()
            .iter()
            .map(|s| s.clap_command())
            .collect();

        let mut base_command = ClapCommand::new(self.name())
            .about(self.about())
            .version(crate_version!())
            .author(crate_authors!());

        if !clap_subcommands.is_empty() {
            base_command = base_command
                .subcommand_required(true)
                .subcommands(clap_subcommands);
        }

        self.configure_args(base_command)
    }

    /// After initializing a [`ClapCommand`] using [Self::clap_command], subcommands may customize
    /// the `ClapCommand` further by overriding this default implementation.
    fn configure_args(&self, command: ClapCommand) -> ClapCommand {
        // Does nothing by default
        command
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

pub trait WithIonCliArgument {
    fn with_input(self) -> Self;
    fn with_output(self) -> Self;
    fn with_format(self) -> Self;
}

impl WithIonCliArgument for ClapCommand {
    fn with_input(self) -> Self {
        self.arg(
            Arg::new("input")
                .index(1)
                .trailing_var_arg(true)
                .action(ArgAction::Append)
                .help("Input file"),
        )
    }

    fn with_output(self) -> Self {
        self.arg(
            Arg::new("output")
                .long("output")
                .short('o')
                .help("Output file [default: STDOUT]"),
        )
    }

    fn with_format(self) -> Self {
        self.arg(
            Arg::new("format")
                .long("format")
                .short('f')
                .default_value("pretty")
                .value_parser(["binary", "text", "pretty", "lines"])
                .help("Output format"),
        )
    }
}

use anyhow::anyhow;
use clap::{crate_authors, crate_version, Arg, ArgAction, ArgMatches, Command as ClapCommand};
pub mod beta;
pub mod dump;

/// Behaviors common to all Ion CLI commands, including both namespaces (groups of commands)
/// and the commands themselves.
pub trait IonCliCommand {
    /// Returns the name of this command.
    ///
    /// This value is used for dispatch (that is: mapping the name provided on the command line
    /// to the appropriate command) and for help messages.
    fn name(&self) -> &'static str;

    /// A brief message describing this command's functionality.
    fn about(&self) -> &'static str;

    /// Initializes a [`ClapCommand`] representing this command and its subcommands (if any).
    ///
    /// Commands wishing to customize their `ClapCommand`'s arguments should override
    /// [`Self::configure_args`].
    fn clap_command(&self) -> ClapCommand {
        // Create a `ClapCommand` representing each of this command's subcommands.
        let clap_subcommands: Vec<_> = self
            .subcommands()
            .iter()
            .map(|s| s.clap_command())
            .collect();

        // Configure a 'base' clap configuration that has the command's name, about message,
        // version, and author.
        let mut base_command = ClapCommand::new(self.name())
            .about(self.about())
            .version(crate_version!())
            .author(crate_authors!());

        // If there are subcommands, add them to the configuration and set 'subcommand_required'.
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

    /// Returns a `Vec` containing all of this command's subcommands.
    ///
    /// Namespaces should override the default implementation to specify their subcommands.
    /// Commands should use the default implementation.
    fn subcommands(&self) -> Vec<Box<dyn IonCliCommand>> {
        Vec::new()
    }

    /// Returns the subcommand that corresponds to the specified name. If no matching subcommand
    /// is found, returns `None`.
    fn get_subcommand(&self, subcommand_name: &str) -> Option<Box<dyn IonCliCommand>> {
        let mut subcommands = self.subcommands();
        if let Some(index) = subcommands.iter().position(|s| s.name() == subcommand_name) {
            Some(subcommands.swap_remove(index))
        } else {
            None
        }
    }

    /// Sets up the pager (e.g. `less`)  to which long text output will be directed. The default
    /// implementation does not configure a pager.
    fn set_up_pager(&self) {}

    /// The core logic of the command.
    ///
    /// The default implementation assumes this command is a namespace (i.e. a group of subcommands).
    /// It looks for a subcommand in the arguments, then looks up and runs that subcommand.
    ///
    /// Commands should override this implementation.
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

/// Extension methods for a [`ClapCommand`] which add flags and options that are common to
/// commands in the Ion CLI.
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

use crate::commands::dump;
use crate::commands::IonCliCommand;
use anyhow::Result;
use clap::{value_parser, Arg, ArgAction, ArgMatches, Command};

pub struct HeadCommand;

impl IonCliCommand for HeadCommand {
    fn name(&self) -> &'static str {
        "head"
    }

    fn about(&self) -> &'static str {
        "Prints the specified number of top-level values in the input stream."
    }

    fn clap_command(&self) -> Command {
        Command::new(self.name())
            .about(self.about())
            .arg(
                Arg::new("values")
                    .long("values")
                    .short('n')
                    .value_parser(value_parser!(usize))
                    .allow_negative_numbers(false)
                    .default_value("10")
                    .help("Specifies the number of output top-level values."),
            )
            .arg(
                Arg::new("format")
                    .long("format")
                    .short('f')
                    .default_value("lines")
                    .value_parser(["binary", "text", "pretty", "lines"])
                    .help("Output format"),
            )
            .arg(
                Arg::new("output")
                    .long("output")
                    .short('o')
                    .help("Output file [default: STDOUT]"),
            )
            .arg(
                // All argv entries after the program name (argv[0])
                // and any `clap`-managed options are considered input files.
                Arg::new("input")
                    .index(1)
                    .help("Input file [default: STDIN]")
                    .action(ArgAction::Append)
                    .trailing_var_arg(true),
            )
    }

    fn run(&self, command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        //TODO: Extract common value-handling logic for both `head` and `dump`
        // https://github.com/amazon-ion/ion-cli/issues/49
        //TODO: Multiple file handling in classic `head` includes a header per file.
        // https://github.com/amazon-ion/ion-cli/issues/48
        dump::run(command_path.last().unwrap(), args)
    }
}

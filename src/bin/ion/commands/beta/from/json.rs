use crate::commands::dump;
use anyhow::Result;
use clap::{Arg, ArgAction, ArgMatches, Command};
use crate::IonCliCommand;

pub struct FromJsonCommand;

impl IonCliCommand for FromJsonCommand {
    fn name(&self) -> &'static str {
        "json"
    }

    fn about(&self) -> &'static str {
        "Converts data from JSON to Ion."
    }

    fn clap_command(&self) -> Command {
        Command::new(self.name())
            .about(self.about())
            .arg(
                Arg::new("output")
                    .long("output")
                    .short('o')
                    .help("Output file [default: STDOUT]"),
            )
            .arg(
                Arg::new("format")
                    .long("format")
                    .short('f')
                    .default_value("pretty")
                    .value_parser(["binary", "text", "pretty", "lines"])
                    .help("Output format"),
            )
            .arg(
                // Any number of input files can be specified by repeating the "-i" or "--input" flags.
                // Unlabeled positional arguments will also be considered input file names.
                Arg::new("input")
                    .index(1)
                    .trailing_var_arg(true)
                    .action(ArgAction::Append)
                    .help("Input file"),
            )
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        // Because JSON data is valid Ion, the `dump` command may be reused for converting JSON.
        // TODO ideally, this would perform some smarter "up-conversion".
        dump::run("json", args)
    }
}

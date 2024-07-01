use anyhow::Result;
use clap::{value_parser, Arg, ArgMatches, Command};
use ion_rs::{AnyEncoding, Reader};

use crate::commands::{CommandIo, IonCliCommand, WithIonCliArgument};
use crate::transcribe::write_n_as;

pub struct HeadCommand;

impl IonCliCommand for HeadCommand {
    fn name(&self) -> &'static str {
        "head"
    }

    fn about(&self) -> &'static str {
        "Prints the specified number of top-level values in the input stream."
    }

    fn configure_args(&self, command: Command) -> Command {
        // Same flags as `cat`, but with an added `--values` flag to specify the number of values to
        // write.
        command.with_input().with_output().with_format().arg(
            Arg::new("values")
                .long("values")
                .short('n')
                .value_parser(value_parser!(usize))
                .allow_negative_numbers(false)
                .default_value("10")
                .help("Specifies the number of output top-level values."),
        )
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        //TODO: Multiple file handling in classic `head` includes a header per file.
        // https://github.com/amazon-ion/ion-cli/issues/48

        // --format pretty|text|lines|binary
        // `clap` validates the specified format and provides a default otherwise.
        let format = args.get_one::<String>("format").unwrap();
        let num_values = *args.get_one::<usize>("values").unwrap();

        CommandIo::new(args).for_each_input(|output, input| {
            let mut reader = Reader::new(AnyEncoding, input.into_source())?;
            write_n_as(&mut reader, output, format, num_values)?;
            Ok(())
        })
    }
}

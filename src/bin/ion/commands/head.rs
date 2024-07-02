use anyhow::{bail, Result};
use clap::{value_parser, Arg, ArgMatches, Command};
use ion_rs::{AnyEncoding, Format, IonEncoding, Reader, TextFormat};

use crate::commands::{CommandIo, IonCliCommand, WithIonCliArgument, ION_VERSION_ARG_ID};
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
        command
            .with_input()
            .with_output()
            .with_format()
            .with_ion_version()
            .arg(
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
        let format: Format = match args.get_one::<String>("format").unwrap().as_str() {
            "text" => Format::Text(TextFormat::Compact),
            "lines" => Format::Text(TextFormat::Lines),
            "pretty" => Format::Text(TextFormat::Pretty),
            "binary" => Format::Binary,
            unrecognized => bail!("unsupported format '{unrecognized}'"),
        };
        let encoding = match (
            args.get_one::<String>(ION_VERSION_ARG_ID).unwrap().as_str(),
            format,
        ) {
            ("1.0", Format::Text(_)) => IonEncoding::Text_1_0,
            ("1.0", Format::Binary) => IonEncoding::Binary_1_0,
            ("1.1", Format::Text(_)) => IonEncoding::Text_1_1,
            ("1.1", Format::Binary) => IonEncoding::Binary_1_1,
            (unrecognized, _) => bail!("unrecognized Ion version '{unrecognized}'"),
        };

        let num_values = *args.get_one::<usize>("values").unwrap();

        CommandIo::new(args).for_each_input(|output, input| {
            let mut reader = Reader::new(AnyEncoding, input.into_source())?;
            write_n_as(&mut reader, output, encoding, format, num_values)?;
            Ok(())
        })
    }
}

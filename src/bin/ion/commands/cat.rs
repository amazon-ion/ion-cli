use anyhow::Result;
use clap::{arg, ArgMatches, Command};
use ion_rs::*;

use crate::commands::timestamp_conversion::convert_timestamps;
use crate::commands::{CommandIo, IonCliCommand, WithIonCliArgument};
use crate::transcribe::{write_all_as, write_all_as_with_mapper};

pub struct CatCommand;

impl IonCliCommand for CatCommand {
    fn name(&self) -> &'static str {
        "cat"
    }

    fn about(&self) -> &'static str {
        "Prints all Ion input files to the specified output in the requested format."
    }

    fn is_stable(&self) -> bool {
        true
    }

    fn is_porcelain(&self) -> bool {
        false
    }

    fn configure_args(&self, command: Command) -> Command {
        command
            .alias("dump")
            .arg(arg!(-t --"detect-timestamps" "Preserve Ion timestamps when going from Ion to JSON to Ion"))
            .with_input()
            .with_output()
            .with_format()
            .with_color()
            .with_ion_version()
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        let detect_timestamps = args.get_flag("detect-timestamps");
        let mapper = if detect_timestamps {
            Some(convert_timestamps as fn(Element) -> Result<Element>)
        } else {
            None
        };

        CommandIo::new(args)?.for_each_input(|output, input| {
            let mut reader = Reader::new(AnyEncoding, input.into_source())?;
            let encoding = *output.encoding();
            let format = *output.format();

            if detect_timestamps {
                write_all_as_with_mapper(&mut reader, output, encoding, format, mapper)?;
            } else {
                write_all_as(&mut reader, output, encoding, format)?;
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_string_conversion() -> Result<()> {
        // Test that timestamp-like strings are converted
        let timestamp_string = Element::from("2023-01-01T00:00:00Z");
        let result = convert_timestamps(timestamp_string)?;
        assert_eq!(result.ion_type(), IonType::Timestamp);

        // Test nested timestamp in struct (tests pre-order traversal)
        let struct_with_timestamp = Element::from(
            ion_rs::Struct::builder()
                .with_field("created", "2025-01-01T12:00:00Z")
                .build(),
        );
        let result = convert_timestamps(struct_with_timestamp)?;
        let created_field = result.as_struct().unwrap().get("created").unwrap();
        assert_eq!(created_field.ion_type(), IonType::Timestamp);

        Ok(())
    }
}

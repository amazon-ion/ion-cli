use anyhow::Result;
use clap::{arg, ArgMatches, Command};
use ion_rs::{AnyEncoding, Element, IonType, Reader};

use crate::commands::{CommandIo, IonCliCommand, WithIonCliArgument};
use crate::input::CommandInput;
use crate::output::CommandOutput;

pub struct FromJsonCommand;

impl IonCliCommand for FromJsonCommand {
    fn name(&self) -> &'static str {
        "json"
    }

    fn about(&self) -> &'static str {
        "Converts data from JSON to Ion."
    }

    fn is_stable(&self) -> bool {
        false // TODO: Should this be true?
    }

    fn is_porcelain(&self) -> bool {
        false
    }

    fn configure_args(&self, command: Command) -> Command {
        command
            .arg(arg!(--"detect-timestamps" "Parse ISO 8601 timestamp strings as Ion timestamps"))
            .with_input()
            .with_output()
            .with_format()
            .with_ion_version()
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        // Because JSON data is valid Ion, the `cat` command may be reused for converting JSON.
        // TODO ideally, this would perform some smarter "up-conversion".
        let detect_timestamps = args.get_flag("detect-timestamps");
        CommandIo::new(args)?
            .for_each_input(|output, input| convert(input, output, detect_timestamps))
    }
}

pub fn convert(
    input: CommandInput,
    output: &mut CommandOutput,
    detect_timestamps: bool,
) -> Result<()> {
    const FLUSH_EVERY_N: usize = 100;
    let mut writer = output.as_writer()?;
    let mut value_count = 0usize;
    let mut ion_reader = Reader::new(AnyEncoding, input.into_source())?;

    while let Some(lazy_value) = ion_reader.next()? {
        let value_ref = lazy_value.read()?;
        let element = Element::try_from(value_ref)?;
        let converted_element = if detect_timestamps {
            convert_timestamps(element)?
        } else {
            element
        };
        writer.write(&converted_element)?;
        value_count += 1;
        if value_count % FLUSH_EVERY_N == 0 {
            writer.flush()?;
        }
    }

    writer.close().map_err(Into::into)
}

fn convert_timestamps(element: Element) -> Result<Element> {
    Ok(match element.ion_type() {
        IonType::String => {
            let s = element.as_string().unwrap();
            if is_timestamp_like(s) {
                if let Ok(timestamp_element) = Element::read_one(s.as_bytes()) {
                    if timestamp_element.ion_type() == IonType::Timestamp {
                        return Ok(timestamp_element);
                    }
                }
            }
            element
        }
        IonType::List => {
            let list = element.as_sequence().unwrap();
            let converted: Result<Vec<_>> = list
                .elements()
                .map(|e| convert_timestamps(e.clone()))
                .collect();
            Element::from(ion_rs::List::from(converted?))
        }
        IonType::Struct => {
            let struct_val = element.as_struct().unwrap();
            let mut struct_builder = ion_rs::Struct::builder();
            for (field, value) in struct_val.fields() {
                struct_builder =
                    struct_builder.with_field(field, convert_timestamps(value.clone())?);
            }
            Element::from(struct_builder.build())
        }
        _ => element,
    })
}

/// Heuristic to identify strings that could be Ion timestamps
///
/// Ion timestamps follow ISO 8601 format with these constraints:
/// - Years 0001-9999 (4 digits)
/// - Precision up to nanoseconds
/// - Must have date component (YYYY, YYYY-MM, or YYYY-MM-DD)
///
/// This function uses position-based checks, which are cheaper compared to string operations:
/// - Length bounds (4-35 chars for timestamp range)
/// - Direct character position checks
fn is_timestamp_like(s: &str) -> bool {
    let len = s.len();

    // Bounds check, timestamps are 4-35 chars
    if !(4..=35).contains(&len) {
        return false;
    }

    // Must start with 4 digits
    let bytes = s.as_bytes();
    if !bytes[0].is_ascii_digit()
        || !bytes[1].is_ascii_digit()
        || !bytes[2].is_ascii_digit()
        || !bytes[3].is_ascii_digit()
    {
        return false;
    }

    match len {
        4 => false,
        5..=9 => bytes[len - 1] == b'T',
        10 => bytes[4] == b'-' && bytes[7] == b'-',
        _ => len > 10 && bytes[10] == b'T',
    }
}

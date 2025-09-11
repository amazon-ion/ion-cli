use anyhow::{Context, Result};
use clap::{arg, ArgMatches, Command};
use ion_rs::Element;
use serde_json::{Deserializer, Value};

use crate::commands::{CommandIo, IonCliCommand, WithIonCliArgument};
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
        CommandIo::new(args)?.for_each_input(|output, input| {
            let input_name = input.name().to_owned();
            convert(input.into_source(), output, detect_timestamps, &input_name)
        })
    }
}

pub fn convert(
    reader: impl std::io::Read,
    output: &mut CommandOutput,
    detect_timestamps: bool,
    input_name: &str,
) -> Result<()> {
    let mut writer = output.as_writer()?;

    // Streaming deserializer to handle large JSON files
    let deserializer = Deserializer::from_reader(reader);

    for (i, json_value) in deserializer.into_iter::<Value>().enumerate() {
        let json_value = json_value
            .with_context(|| format!("Input file '{}' contains invalid JSON.", input_name))?;
        writer.write(&to_ion_element(json_value, detect_timestamps)?)?;
        // Periodic flushing
        if i % 100 == 99 {
            writer.flush()?;
        }
    }

    writer.close().map_err(Into::into)
}

fn to_ion_element(value: Value, detect_timestamps: bool) -> Result<Element> {
    Ok(match value {
        Value::Null => Element::null(ion_rs::IonType::Null),
        Value::Bool(b) => Element::from(b),
        Value::Number(n) => {
            // Preserve integer precision when possible
            // fall back to float
            if let Some(i) = n.as_i64() {
                Element::from(i)
            } else {
                Element::from(n.as_f64().unwrap())
            }
        }
        Value::String(s) => {
            if detect_timestamps && is_timestamp_like(&s) {
                if let Ok(element) = Element::read_one(s.as_bytes()) {
                    if element.ion_type() == ion_rs::IonType::Timestamp {
                        return Ok(element);
                    }
                }
            }
            Element::from(s)
        }
        Value::Array(arr) => {
            let elements: Result<Vec<_>> = arr
                .into_iter()
                .map(|v| to_ion_element(v, detect_timestamps))
                .collect();
            Element::from(ion_rs::List::from(elements?))
        }
        Value::Object(obj) => {
            let mut struct_builder = ion_rs::Struct::builder();
            for (key, val) in obj {
                struct_builder =
                    struct_builder.with_field(key, to_ion_element(val, detect_timestamps)?);
            }
            Element::from(struct_builder.build())
        }
    })
}

fn is_timestamp_like(s: &str) -> bool {
    s.len() >= 4 
        && s[..4].chars().all(|c| c.is_ascii_digit())
        && (s.contains('T') || (s.len() == 10 && s.matches('-').count() == 2))
        && !(s.len() == 4 && s.chars().all(|c| c.is_ascii_digit()))
}

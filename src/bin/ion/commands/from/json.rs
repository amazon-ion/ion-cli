use anyhow::{Context, Result};
use clap::{arg, ArgMatches, Command};
use ion_rs::{Element, Timestamp};
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
        // Args must be identical to CatCommand so that we can safely delegate
        command
            .arg(arg!(--"detect-timestamps" "Parse ISO 8601 timestamp strings as Ion timestamps (not yet implemented)"))
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
            let reader = input.into_source();
            convert(reader, output, detect_timestamps, &input_name)
        })
    }
}

pub fn convert(
    reader: impl std::io::Read,
    output: &mut CommandOutput,
    detect_timestamps: bool,
    input_name: &str,
) -> Result<()> {
    const FLUSH_EVERY_N: usize = 100;
    let mut value_count = 0usize;
    let mut writer = output.as_writer()?;

    // Streaming deserializer to handle large JSON files
    let deserializer = Deserializer::from_reader(reader);

    for json_value in deserializer.into_iter::<Value>() {
        let json_value = json_value
            .with_context(|| format!("Input file '{}' contains invalid JSON.", input_name))?;
        let ion_element = to_ion_element(json_value, detect_timestamps)?;
        writer.write(&ion_element)?;
        value_count += 1;

        // Periodic flushing
        if value_count % FLUSH_EVERY_N == 0 {
            writer.flush()?;
        }
    }

    writer.close()?;
    Ok(())
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
                // Fall back if timestamp parsing fails
                if let Ok(timestamp) = parse_timestamp(&s) {
                    return Ok(Element::from(timestamp));
                }
            }
            Element::from(s)
        }
        Value::Array(arr) => {
            let elements: Result<Vec<Element>> = arr
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

fn parse_timestamp(s: &str) -> Result<Timestamp> {
    let Some((date_part, time_part)) = s.split_once('T') else {
        return Err(anyhow::anyhow!("Not a valid ISO 8601 datetime format"));
    };

    let date_parts: Vec<&str> = date_part.split('-').collect();
    if date_parts.len() != 3 {
        return Err(anyhow::anyhow!("Invalid date format"));
    }

    let year: u32 = date_parts[0].parse()?;
    let month: u32 = date_parts[1].parse()?;
    let day: u32 = date_parts[2].parse()?;

    let time_clean = time_part.trim_end_matches('Z');
    let time_str = if let Some(pos) = time_clean.find('+') {
        &time_clean[..pos]
    } else if let Some(pos) = time_clean.rfind('-').filter(|&i| i > 2) {
        &time_clean[..pos]
    } else {
        time_clean
    };

    let time_parts: Vec<&str> = time_str.split(':').collect();
    if time_parts.len() < 2 {
        return Err(anyhow::anyhow!("Invalid time format"));
    }

    let hour: u32 = time_parts[0].parse()?;
    let minute: u32 = time_parts[1].parse()?;
    let second: u32 = if time_parts.len() > 2 {
        time_parts[2]
            .split('.')
            .next()
            .unwrap_or("0")
            .parse()
            .unwrap_or(0)
    } else {
        0
    };

    // Building the Ion timestamp using builder patterns
    Timestamp::with_year(year)
        .with_month(month)
        .with_day(day)
        .with_hour_and_minute(hour, minute)
        .with_second(second)
        .build()
        .map_err(Into::into)
}

fn is_timestamp_like(s: &str) -> bool {
    // Heuristic to avoid parsing on non-timestamp strings
    s.len() >= 19 && s.contains('T') && s.matches('-').count() >= 2 && s.contains(':')
}

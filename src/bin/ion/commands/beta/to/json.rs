use crate::commands::{IonCliCommand, WithIonCliArgument};
use anyhow::{Context, Result};
use clap::{ArgMatches, Command};
use ion_rs::{Element, ElementReader};
use ion_rs::{Reader, ReaderBuilder};
use serde_json::{Map, Number, Value as JsonValue};
use std::fs::File;
use std::io::{stdin, stdout, BufWriter, Write};
use std::str::FromStr;

pub struct ToJsonCommand;

impl IonCliCommand for ToJsonCommand {
    fn name(&self) -> &'static str {
        "json"
    }

    fn about(&self) -> &'static str {
        "Converts Ion data to JSON."
    }

    fn configure_args(&self, command: Command) -> Command {
        // NOTE: it may be necessary to add format-specific options. For example, a "pretty" option
        // would make sense for JSON, but not binary formats like CBOR.
        command.with_input().with_output()
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        // Look for an output file name specified with `-o`
        let mut output: Box<dyn Write> = if let Some(output_file) = args.get_one::<String>("output")
        {
            let file = File::create(output_file).with_context(|| {
                format!(
                    "could not open file output file '{}' for writing",
                    output_file
                )
            })?;
            Box::new(BufWriter::new(file))
        } else {
            Box::new(stdout().lock())
        };

        if let Some(input_file_names) = args.get_many::<String>("input") {
            // Input files were specified, run the converter on each of them in turn
            for input_file in input_file_names {
                let file = File::open(input_file.as_str())
                    .with_context(|| format!("Could not open file '{}'", &input_file))?;
                let mut reader = ReaderBuilder::new()
                    .build(file)
                    .with_context(|| format!("Input file {} was not valid Ion.", &input_file))?;
                convert(&mut reader, &mut output)?;
            }
        } else {
            // No input files were specified, run the converter on STDIN.
            let mut reader = ReaderBuilder::new()
                .build(stdin().lock())
                .with_context(|| "Input was not valid Ion.")?;
            convert(&mut reader, &mut output)?;
        }

        output.flush()?;
        Ok(())
    }
}

pub fn convert(reader: &mut Reader, output: &mut Box<dyn Write>) -> Result<()> {
    const FLUSH_EVERY_N: usize = 100;
    let mut element_count = 0usize;
    for result in reader.elements() {
        let element = result.with_context(|| "invalid input")?;
        writeln!(output, "{}", to_json_value(&element)?)?;
        element_count += 1;
        if element_count % FLUSH_EVERY_N == 0 {
            output.flush()?;
        }
    }
    Ok(())
}

fn to_json_value(element: &Element) -> Result<JsonValue> {
    if element.is_null() {
        Ok(JsonValue::Null)
    } else {
        use ion_rs::Value::*;
        let value = match element.value() {
            Null(_ion_type) => JsonValue::Null,
            Bool(b) => JsonValue::Bool(*b),
            Int(i) => JsonValue::Number(
                Number::from_str(&(*i).to_string())
                    .with_context(|| format!("{element} could not be turned into a Number"))?,
            ),
            Float(f) => {
                let value = *f;
                if value.is_finite() {
                    JsonValue::Number(
                        Number::from_f64(value).with_context(|| {
                            format!("{element} could not be turned into a Number")
                        })?,
                    )
                } else {
                    // +inf, -inf, and nan are not JSON numbers, and are written as null in
                    // accordance with Ion's JSON down-conversion guidelines.
                    JsonValue::Null
                }
            }
            Decimal(d) => JsonValue::Number(
                Number::from_str(d.to_string().replace('d', "e").as_str())
                    .with_context(|| format!("{element} could not be turned into a Number"))?,
            ),
            Timestamp(t) => JsonValue::String(t.to_string()),
            Symbol(s) => s
                .text()
                .map(|text| JsonValue::String(text.to_owned()))
                .unwrap_or_else(|| JsonValue::Null),
            String(s) => JsonValue::String(s.text().to_owned()),
            Blob(b) | Clob(b) => {
                use base64::{engine::general_purpose as base64_encoder, Engine as _};
                let base64_text = base64_encoder::STANDARD.encode(b.as_ref());
                JsonValue::String(base64_text)
            }
            List(s) | SExp(s) => {
                let result: Result<Vec<JsonValue>> = s.elements().map(to_json_value).collect();
                JsonValue::Array(result?)
            }
            Struct(s) => {
                let result: Result<Map<std::string::String, JsonValue>> = s
                    .fields()
                    .map(|(k, v)| to_json_value(v).map(|value| (k.text().unwrap().into(), value)))
                    .collect();
                JsonValue::Object(result?)
            }
        };
        Ok(value)
    }
}

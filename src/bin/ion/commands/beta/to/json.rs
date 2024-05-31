use crate::commands::{IonCliCommand, WithIonCliArgument};
use anyhow::{Context, Result};
use clap::{ArgMatches, Command};
use serde_json::{Map, Number, Value as JsonValue};
use std::fs::File;
use std::io::{stdin, stdout, BufWriter, Write};
use std::str::FromStr;
use new_ion_rs::*;

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
                let mut reader = Reader::new(AnyEncoding, file)
                    .with_context(|| format!("Input file {} was not valid Ion.", &input_file))?;
                convert(&mut reader, &mut output)?;
            }
        } else {
            // No input files were specified, run the converter on STDIN.
            let mut reader = Reader::new(AnyEncoding, stdin().lock())
                .with_context(|| "Input was not valid Ion.")?;
            convert(&mut reader, &mut output)?;
        }

        output.flush()?;
        Ok(())
    }
}

pub fn convert(reader: &mut Reader<AnyEncoding, impl IonInput>, output: &mut Box<dyn Write>) -> Result<()> {
    const FLUSH_EVERY_N: usize = 100;
    let mut value_count = 0usize;
    while let Some(value) = reader.next()? {
        writeln!(output, "{}", to_json_value(value)?)?;
        value_count += 1;
        if value_count % FLUSH_EVERY_N == 0 {
            output.flush()?;
        }
    }
    Ok(())
}

fn to_json_value(value: LazyValue<AnyEncoding>) -> Result<JsonValue> {
    use ValueRef::*;
    let value = match value.read()? {
        Null(_) => JsonValue::Null,
        Bool(b) => JsonValue::Bool(b),
        Int(i) => JsonValue::Number(Number::from(i.expect_i128()?)),
        Float(f) if f.is_finite() => JsonValue::Number(Number::from_f64(f).expect("f64 is finite")),
        // Special floats like +inf, -inf, and NaN are written as `null` in
        // accordance with Ion's JSON down-conversion guidelines.
        Float(_f) => JsonValue::Null,
        Decimal(d) => {
            let mut text = d.to_string().replace('d', "e");
            if text.ends_with(".") {
                // If there's a trailing "." with no digits of precision, discard it. JSON's `Number`
                // type does not do anything with this information.
                let _ = text.pop();
            }
            JsonValue::Number(
                Number::from_str(text.as_str())
                    .with_context(|| format!("{d} could not be turned into a Number"))?,
            )
        }
        Timestamp(t) => JsonValue::String(t.to_string()),
        Symbol(s) => s.text()
            .map(|text| JsonValue::String(text.to_owned()))
            .unwrap_or_else(|| JsonValue::Null),
        String(s) => JsonValue::String(s.text().to_owned()),
        Blob(b) | Clob(b) => {
            use base64::{engine::general_purpose as base64_encoder, Engine as _};
            let base64_text = base64_encoder::STANDARD.encode(b.as_ref());
            JsonValue::String(base64_text)
        }
        SExp(s) => to_json_array(s.iter())?,
        List(l) => to_json_array(l.iter())?,
        Struct(s) => {
            let mut map = Map::new();
            for field in s {
                let field = field?;
                let name = field.name()?.text().unwrap_or("$0").to_owned();
                let value = to_json_value(field.value())?;
                map.insert(name, value);
            }
            JsonValue::Object(map)
        }
    };
    Ok(value)
}

fn to_json_array<'a>(ion_values: impl IntoIterator<Item=IonResult<LazyValue<'a, AnyEncoding>>>) -> Result<JsonValue> {
    let result: Result<Vec<JsonValue>> = ion_values.into_iter().flat_map(|v| v.map(to_json_value)).collect();
    Ok(JsonValue::Array(result?))
}

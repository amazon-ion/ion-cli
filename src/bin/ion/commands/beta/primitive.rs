use crate::commands::CommandConfig;
use anyhow::{Context, Result};
use clap::{App, Arg, ArgMatches};
use ion_rs::binary::var_int::VarInt;
use ion_rs::binary::var_uint::VarUInt;

pub fn app() -> CommandConfig {
    App::new("primitive")
        .about("Prints the binary representation of an Ion encoding primitive.")
        .arg(
            Arg::with_name("type")
                .short("t")
                .required(true)
                .takes_value(true)
                .help("The Ion primitive encoding type. (Names are case insensitive.)")
                .possible_values(&["VarInt", "varint", "VarUInt", "varuint"]),
        )
        .arg(
            Arg::with_name("value")
                .short("v")
                .required(true)
                .allow_hyphen_values(true)
                .takes_value(true)
                .help("The value to encode as the specified primitive."),
        )
}

pub fn run(_command_name: &str, matches: &ArgMatches<'static>) -> anyhow::Result<()> {
    let mut buffer = Vec::new();
    let value_text = matches.value_of("value").unwrap();
    match matches.value_of("type").unwrap() {
        "varuint" | "VarUInt" => {
            let value = integer_from_text(value_text)? as u64;
            VarUInt::write_u64(&mut buffer, value).unwrap();
        }
        "varint" | "VarInt" => {
            let value = integer_from_text(value_text)?;
            VarInt::write_i64(&mut buffer, value).unwrap();
        }
        unsupported => {
            unreachable!(
                "clap did not reject unsupported primitive encoding {}",
                unsupported
            );
        }
    }
    print!("hex: ");
    for byte in buffer.iter() {
        // We want the hex bytes to align with the binary bytes that will be printed on the next
        // line. Print 6 spaces and a 2-byte hex representation of the byte.
        print!("      {:0>2x} ", byte);
    }
    println!();
    print!("bin: ");
    for byte in buffer.iter() {
        // Print the binary representation of each byte
        print!("{:0>8b} ", byte);
    }
    println!();
    Ok(())
}

fn integer_from_text(text: &str) -> Result<i64> {
    if text.starts_with("0x") {
        i64::from_str_radix(text, 16)
            .with_context(|| format!("{} is not a valid hexidecimal integer value.", text))
    } else if text.starts_with("0b") {
        i64::from_str_radix(text, 2)
            .with_context(|| format!("{} is not a valid binary integer value.", text))
    } else {
        text.parse::<i64>()
            .with_context(|| format!("{} is not a valid decimal integer value.", text))
    }
}

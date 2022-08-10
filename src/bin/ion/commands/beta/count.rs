use crate::commands::CommandConfig;
use anyhow::{Context, Result};
use clap::{App, Arg, ArgMatches};
use ion_rs::*;
use std::fs::File;
use std::io::{stdin, BufReader, StdinLock};

pub fn app() -> CommandConfig {
    App::new("count")
        .about("Prints the number of top-level values found in the input stream.")
        .arg(
            // All argv entries after the program name (argv[0])
            // and any `clap`-managed options are considered input files.
            Arg::with_name("input")
                .index(1)
                .multiple(true)
                .help("Input file [default: STDIN]"),
        )
}

pub fn run(_command_name: &str, matches: &ArgMatches<'static>) -> Result<()> {
    if let Some(input_file_iter) = matches.values_of("input") {
        for input_file in input_file_iter {
            let file = File::open(input_file)
                .with_context(|| format!("Could not open file '{}'", input_file))?;
            let mut reader = ReaderBuilder::new().build(file)?;
            print_top_level_value_count(&mut reader)?;
        }
    } else {
        let input: StdinLock = stdin().lock();
        let buf_reader = BufReader::new(input);
        let mut reader = ReaderBuilder::new().build(buf_reader)?;
        print_top_level_value_count(&mut reader)?;
    };

    Ok(())
}

fn print_top_level_value_count(reader: &mut Reader) -> Result<()> {
    let mut count: usize = 0;
    loop {
        let item = reader
            .next()
            .with_context(|| "could not count values in Ion stream")?;
        if item == StreamItem::Nothing {
            break;
        }
        count += 1;
    }
    println!("{}", count);
    Ok(())
}

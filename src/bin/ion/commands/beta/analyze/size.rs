use crate::commands::{IonCliCommand, WithIonCliArgument};
use anyhow::{bail, Context, Result};
use clap::{ArgMatches, Command};
use ion_rs::{IonReader, RawBinaryReader, SystemReader, SystemStreamItem};
use lowcharts::plot;
use memmap::MmapOptions;
use std::fs::File;

pub struct SizeCommand;

impl IonCliCommand for SizeCommand {
    fn name(&self) -> &'static str {
        "size"
    }

    fn about(&self) -> &'static str {
        "Prints the overall min, max and mean size of top-level values and plot the size distribution of the input stream."
    }

    fn configure_args(&self, command: Command) -> Command {
        command.with_input()
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        if let Some(input_file_names) = args.get_many::<String>("input") {
            for input_file in input_file_names {
                let file = File::open(input_file.as_str())
                    .with_context(|| format!("Could not open file '{}'", &input_file))?;
                let mmap = unsafe {
                    MmapOptions::new()
                        .map(&file)
                        .with_context(|| format!("Could not mmap '{}'", input_file))?
                };
                // Treat the mmap as a byte array.
                let ion_data: &[u8] = &mmap[..];
                let raw_reader = RawBinaryReader::new(ion_data);
                let mut system_reader = SystemReader::new(raw_reader);
                size_analyze(&mut system_reader)
                    .expect("Failed to analyze the size of the input data.");
            }
        } else {
            bail!("this command does not yet support reading from STDIN")
        }
        Ok(())
    }
}

fn size_analyze(reader: &mut SystemReader<RawBinaryReader<&[u8]>>) -> Result<()> {
    let mut size_vec: Vec<f64> = Vec::new();
    loop {
        match reader.next()? {
            SystemStreamItem::Value(_) => {
                let size = reader.annotations_length().map_or(
                    reader.header_length() + reader.value_length(),
                    |annotations_length| {
                        annotations_length + reader.header_length() + reader.value_length()
                    },
                );
                size_vec.push(size as f64);
            }
            SystemStreamItem::Nothing => break,
            _ => {}
        }
    }
    // Plot a histogram of the above vector, with 4 buckets and a precision
    // chosen by library. The number of buckets could be changed as needed.
    let options = plot::HistogramOptions {
        intervals: 4,
        ..Default::default()
    };
    let histogram = plot::Histogram::new(&size_vec, options);
    print!("{}", histogram);
    Ok(())
}

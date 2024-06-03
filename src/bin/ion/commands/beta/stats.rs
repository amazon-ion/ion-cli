use crate::commands::{IonCliCommand, WithIonCliArgument};
use anyhow::{bail, Context, Result};
use clap::{ArgMatches, Command};
use ion_rs::{
    BinaryWriterBuilder, Element, ElementReader, IonReader, IonType, IonWriter, RawBinaryReader,
    Reader, ReaderBuilder, SystemReader, SystemStreamItem,
};
use lowcharts::plot;
use memmap::MmapOptions;
use std::fs::File;

pub struct StatsCommand;

impl IonCliCommand for StatsCommand {
    fn name(&self) -> &'static str {
        "stats"
    }

    fn about(&self) -> &'static str {
        "Print the analysis report of the input data stream, including the total number of top-level values, their minimum, maximum, and mean sizes, \n\
        and plot the size distribution of the input stream. The report should also include the number of symbol tables in the input stream, \n\
        the total number of different symbols that occurred in the input stream, and the maximum depth of the input data stream. \n\
        Currently, this subcommand only supports data analysis on binary Ion data."
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
                match ion_data {
                    // Pattern match the byte array to verify it starts with an IVM
                    // Currently the 'stats' subcommand only support binary Ion data stream.
                    [0xE0, 0x01, 0x00, 0xEA, ..] => {
                        // TODO: When there is a new release of LazySystemReader which supports accessing the underlying stream data, \n
                        //  we should only initialize LazySystemReader to get all the statistical information instead of initializing user\n
                        //  reader and system reader for retrieving different information.
                        // Initialize user reader for maximum depth calculation.
                        let reader = ReaderBuilder::new().build(file)?;
                        // Initialize system reader to get the information of symbol tables and value length.
                        let raw_reader = RawBinaryReader::new(ion_data);
                        let mut system_reader = SystemReader::new(raw_reader);
                        // Generate data analysis report.
                        analyze(&mut system_reader, reader, &mut std::io::stdout())
                            .expect("Failed to analyze the input data stream.");
                    }
                    _ => {
                        bail!(
                            "Input file '{}' does not appear to be binary Ion.",
                            input_file
                        );
                    }
                };
            }
        } else {
            bail!("this command does not yet support reading from STDIN")
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Output {
    size_vec: Vec<f64>,
    symtab_count: i32,
    symbols_count: usize,
    max_depth: usize,
}

fn analyze_data_stream(
    system_reader: &mut SystemReader<RawBinaryReader<&[u8]>>,
    reader: Reader,
) -> Output {
    let mut size_vec: Vec<f64> = Vec::new();
    let mut symtab_count = 0;
    loop {
        let system_value = system_reader.next().unwrap();
        match system_value {
            SystemStreamItem::SymbolTableValue(IonType::Struct) => {
                symtab_count += 1;
            }
            SystemStreamItem::Value(..) => {
                let size = system_reader.annotations_length().map_or(
                    system_reader.header_length() + system_reader.value_length(),
                    |annotations_length| {
                        annotations_length
                            + system_reader.header_length()
                            + system_reader.value_length()
                    },
                );
                size_vec.push(size as f64);
            }
            SystemStreamItem::Nothing => break,
            _ => {}
        }
    }
    // Reduce the number of shared symbols.
    let symbols_count = system_reader.symbol_table().symbols().iter().len() - 10;
    let max_depth = get_max_depth(reader);

    let out = Output {
        size_vec,
        symtab_count,
        symbols_count,
        max_depth,
    };
    return out;
}

fn get_max_depth(mut reader: Reader) -> usize {
    reader
        .elements()
        .map(|element| calculate_top_level_max_depth(&element.unwrap(), 0))
        .max()
        .unwrap_or(0)
}

fn calculate_top_level_max_depth(element: &Element, depth: usize) -> usize {
    return if element.ion_type().is_container() {
        if element.ion_type() == IonType::Struct {
            element
                .as_struct()
                .unwrap()
                .iter()
                .map(|(_field_name, e)| calculate_top_level_max_depth(e, depth + 1))
                .max()
                .unwrap_or(depth)
        } else {
            element
                .as_sequence()
                .unwrap()
                .into_iter()
                .map(|e| calculate_top_level_max_depth(e, depth + 1))
                .max()
                .unwrap_or(depth)
        }
    } else {
        depth
    };
}

fn analyze(
    system_reader: &mut SystemReader<RawBinaryReader<&[u8]>>,
    reader: Reader,
    mut writer: impl std::io::Write,
) -> Result<()> {
    let out = analyze_data_stream(system_reader, reader);
    // Plot a histogram of the above vector, with 4 buckets and a precision
    // chosen by library. The number of buckets could be changed as needed.
    let options = plot::HistogramOptions {
        intervals: 4,
        ..Default::default()
    };
    let histogram = plot::Histogram::new(&out.size_vec, options);
    writeln!(
        writer,
        "The 'samples' field represents the total number of top-level value of input data stream. The unit of min, max ,avg size is bytes.\n\
        {}",
        histogram
    )
    .expect("There is an error occurred while plotting the size distribution of input data stream.");
    writeln!(writer, "The number of symbols is {} ", out.symbols_count)
        .expect("There is an error occurred while writing the symbols_count.");
    writeln!(
        writer,
        "The number of local symbol tables is {} ",
        out.symtab_count
    )
    .expect("There is an error occurred while writing the symtab_count.");
    writeln!(
        writer,
        "The maximum depth of the input data stream is {}",
        out.max_depth
    )
    .expect("There is an error occurred while writing the max_depth.");
    Ok(())
}

#[test]
fn test_analyze() -> Result<()> {
    let expect_out = Output {
        size_vec: Vec::from([10.0, 15.0, 6.0, 6.0]),
        symtab_count: 4,
        symbols_count: 8,
        max_depth: 2,
    };
    let test_data: &str = r#"
    {
        foo: bar,
        abc: [123, 456]
    }
    {
        foo: baz,
        abc: [42.0, 43e0]
    }
    {
        foo: bar,
        test: data
    }
    {
        foo: baz,
        type: struct
    }
    "#;
    let binary_buffer = {
        let mut buffer = Vec::new();
        let mut writer = BinaryWriterBuilder::new().build(&mut buffer)?;
        for element in ReaderBuilder::new().build(test_data.as_bytes())?.elements() {
            element?.write_to(&mut writer)?;
            writer.flush()?;
        }
        buffer
    };

    let mut system_reader = SystemReader::new(RawBinaryReader::new(binary_buffer.as_slice()));
    let reader = ReaderBuilder::new().build(binary_buffer.as_slice())?;
    let output = analyze_data_stream(&mut system_reader, reader);

    assert_eq!(output, expect_out);
    Ok(())
}

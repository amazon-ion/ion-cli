use crate::commands::{CommandIo, IonCliCommand, WithIonCliArgument};
use anyhow::Result;
use clap::{ArgMatches, Command};
use ion_rs::*;
use ion_rs::{AnyEncoding, IonInput, IonType, SystemReader, SystemStreamItem};
use lowcharts::plot;
use std::cmp::max;

pub struct StatsCommand;

impl IonCliCommand for StatsCommand {
    fn is_porcelain(&self) -> bool {
        false
    }

    fn name(&self) -> &'static str {
        "stats"
    }

    fn is_stable(&self) -> bool {
        false
    }

    fn about(&self) -> &'static str {
        "Print statistics about an Ion stream"
    }

    fn configure_args(&self, command: Command) -> Command {
        command
            .long_about("Print the analysis report of the input data stream, including the total number of\n\
        top-level values, their minimum, maximum, and mean sizes, and plot the size distribution of\n\
        the input stream. The report should also include the number of symbol tables in the input\n\
        stream, the total number of different symbols that occurred in the input stream, and the\n\
        maximum depth of the input data stream. Currently, this subcommand only supports data\n\
        analysis on binary Ion data.")
            .with_input()
    }
    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        CommandIo::new(args).for_each_input(|_output, input| {
            let mut reader = SystemReader::new(AnyEncoding, input.into_source());
            analyze(&mut reader, &mut std::io::stdout())
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Output {
    size_vec: Vec<f64>,
    symtab_count: i32,
    symbols_count: usize,
    max_depth: usize,
}

fn analyze<Input: IonInput>(
    reader: &mut SystemReader<AnyEncoding, Input>,
    mut writer: impl std::io::Write,
) -> Result<()> {
    let out = analyze_data_stream(reader);
    // Plot a histogram of the above vector, with 4 buckets and a precision
    // chosen by library. The number of buckets could be changed as needed.
    let options = plot::HistogramOptions {
        intervals: 4,
        ..Default::default()
    };
    let histogram = plot::Histogram::new(&out.size_vec, options);
    writeln!(
        writer,
        "The 'samples' field represents the total number of top-level value of input data stream. The unit of min, max, avg size is bytes.\n\
        {}",
        histogram
    )
        .expect("There is an error occurred while plotting the size distribution of input data stream.");
    writeln!(writer, "Symbols: {} ", out.symbols_count)
        .expect("There is an error occurred while writing the symbols_count.");
    writeln!(writer, "Local symbol tables: {} ", out.symtab_count)
        .expect("There is an error occurred while writing the symtab_count.");
    writeln!(writer, "Maximum container depth: {}", out.max_depth)
        .expect("There is an error occurred while writing the max_depth.");
    Ok(())
}

fn analyze_data_stream<Input: IonInput>(reader: &mut SystemReader<AnyEncoding, Input>) -> Output {
    let mut size_vec: Vec<f64> = Vec::new();
    let mut symtab_count = 0;
    let mut max_depth = 0;
    loop {
        let system_value = reader.next_item().unwrap();
        match system_value {
            SystemStreamItem::SymbolTable(_) => {
                symtab_count += 1;
            }
            SystemStreamItem::Value(value) => {
                let size = system_value.raw_stream_item().unwrap().span().bytes().len();
                size_vec.push(size as f64);
                let current_depth = top_level_max_depth(value);
                max_depth = max(max_depth, current_depth);
            }
            SystemStreamItem::EndOfStream(_) => {
                break;
            }
            _ => {}
        }
    }
    // Reduce the number of shared symbols.
    let symbols_count = reader.symbol_table().symbols().iter().len() - 10;

    Output {
        size_vec,
        symtab_count,
        symbols_count,
        max_depth,
    }
}

fn top_level_max_depth(value: LazyValue<AnyEncoding>) -> usize {
    let mut max_depth = 0;
    let mut stack = vec![(value, 0)];
    while let Some((current_value, depth)) = stack.pop() {
        max_depth = max(max_depth, depth);
        if current_value.ion_type() == IonType::Struct {
            let struct_value = current_value.read().unwrap().expect_struct().unwrap();
            for field in struct_value {
                stack.push((field.unwrap().value(), depth + 1));
            }
        } else if current_value.ion_type() == IonType::List {
            let list_value = current_value.read().unwrap().expect_list().unwrap();
            for element in list_value {
                stack.push((element.unwrap(), depth + 1));
            }
        } else if current_value.ion_type() == IonType::SExp {
            let list_value = current_value.read().unwrap().expect_sexp().unwrap();
            for element in list_value {
                stack.push((element.unwrap(), depth + 1));
            }
        }
    }
    max_depth
}

#[test]
fn test_analyze() -> Result<()> {
    let expect_out = Output {
        // The expected size values are generated from ion inspect.
        size_vec: Vec::from([11.0, 16.0, 7.0, 7.0]),
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

    let buffer = {
        let mut buffer = Vec::new();
        let mut writer = Writer::new(v1_0::Binary, &mut buffer)?;
        for element in Reader::new(AnyEncoding, test_data.as_bytes())?.elements() {
            writer.write_element(&element.unwrap())?;
            writer.flush()?;
        }
        buffer
    };
    let mut reader = SystemReader::new(AnyEncoding, buffer.as_slice());
    let output = analyze_data_stream(&mut reader);
    assert_eq!(output, expect_out);
    Ok(())
}

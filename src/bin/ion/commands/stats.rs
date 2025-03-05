use crate::commands::{CommandIo, IonCliCommand, WithIonCliArgument};
use anyhow::Result;
use clap::{Arg, ArgMatches, Command};
use ion_rs::*;
use ion_rs::{AnyEncoding, IonInput, SystemReader, SystemStreamItem};
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
            .with_output()
            .arg(
                Arg::new("count")
                    .long("count")
                    .short('n')
                    .num_args(0)
                    .help("Emit only the count of items for each supplied stream"),
            )
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        CommandIo::new(args).for_each_input(|_output, input| {
            let mut reader = SystemReader::new(AnyEncoding, input.into_source());
            analyze(&mut reader, &mut std::io::stdout(), args)
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
struct StreamStats {
    size_vec: Vec<f64>,
    symtab_count: i32,
    symbols_count: usize,
    max_depth: usize,
    unparseable_count: usize,
}

fn analyze<Input: IonInput>(
    reader: &mut SystemReader<AnyEncoding, Input>,
    mut writer: impl std::io::Write,
    args: &ArgMatches,
) -> Result<()> {
    let stats = analyze_data_stream(reader)?;
    // Plot a histogram of the above vector, with 4 buckets and a precision
    // chosen by library. The number of buckets could be changed as needed.
    let options = plot::HistogramOptions {
        intervals: 4,
        ..Default::default()
    };
    let histogram = plot::Histogram::new(&stats.size_vec, options);

    if args.get_flag("count") {
        writeln!(writer, "{}", stats.size_vec.len())?;
        return Ok(());
    } else {
        writeln!(
            writer,
            "'samples' is the number of top-level values for the input stream."
        )?;
        writeln!(writer, "The unit of min, max, and avg size is bytes.")?;
        writeln!(writer, "{}", histogram)?;
        writeln!(writer, "Symbols: {} ", stats.symbols_count)?;
        writeln!(writer, "Local symbol tables: {} ", stats.symtab_count)?;
        writeln!(writer, "Maximum container depth: {}", stats.max_depth)?;
        if stats.unparseable_count > 0 {
            writeln!(writer, "Unparseable values: {}", stats.unparseable_count)?;
        }
    }

    Ok(())
}

fn analyze_data_stream<Input: IonInput>(
    reader: &mut SystemReader<AnyEncoding, Input>,
) -> Result<StreamStats> {
    let mut size_vec: Vec<f64> = Vec::new();
    let mut symtab_count = 0;
    let mut max_depth = 0;
    let mut unparseable_count = 0;

    loop {
        let system_result = reader.next_item();
        use SystemStreamItem::*;

        match system_result {
            Err(e) => {
                unparseable_count += 1;
                if matches!(e, IonError::Incomplete(..)) {
                    break;
                }
            }

            Ok(item) => match item {
                EndOfStream(_) => break,
                VersionMarker(_) | EncodingDirective(_) => continue,
                SymbolTable(_) => symtab_count += 1,
                system_value @ Value(raw_value) => {
                    let size = system_value
                        .raw_stream_item()
                        .map(|v| v.span().bytes().len())
                        .unwrap_or(0); // 1.1 values may not have any physical representation
                    size_vec.push(size as f64);
                    let current_depth = top_level_max_depth(raw_value)?;
                    max_depth = max(max_depth, current_depth);
                }
                // SystemStreamItem is non_exhaustive
                unsupported => panic!("Unsupported system stream item: {unsupported:?}"),
            },
        }
    }

    // Reduce the number of shared symbols.
    let version = reader.detected_encoding().version();
    let system_symbols_offset = if version == IonVersion::v1_0 {
        version.system_symbol_table().len()
    } else {
        0 // 1.1 system symbols are opt-in, it's fair to count them if they are present
    };

    let symbols_count = reader.symbol_table().len() - system_symbols_offset;

    Ok(StreamStats {
        size_vec,
        symtab_count,
        symbols_count,
        max_depth,
        unparseable_count,
    })
}

fn top_level_max_depth(value: LazyValue<AnyEncoding>) -> Result<usize> {
    let mut max_depth = 0;
    let mut stack = vec![(value, 0)];
    while let Some((current_value, depth)) = stack.pop() {
        max_depth = max(max_depth, depth);
        use ValueRef::*;
        match current_value.read()? {
            Struct(s) => {
                for field in s {
                    stack.push((field?.value(), depth + 1));
                }
            }
            List(s) => {
                for element in s {
                    stack.push((element?, depth + 1));
                }
            }
            SExp(s) => {
                for element in s {
                    stack.push((element?, depth + 1));
                }
            }
            _ => continue,
        }
    }
    Ok(max_depth)
}

#[test]
fn test_analyze() -> Result<()> {
    let expect_out = StreamStats {
        // The expected size values are generated from ion inspect.
        size_vec: Vec::from([11.0, 16.0, 7.0, 7.0]),
        symtab_count: 4,
        symbols_count: 8,
        max_depth: 2,
        unparseable_count: 0,
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
    let stats = analyze_data_stream(&mut reader)?;
    assert_eq!(stats, expect_out);
    Ok(())
}

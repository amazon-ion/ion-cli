use anyhow::Result;
use clap::{Arg, ArgAction, ArgMatches, Command};
use ion_rs::*;
use std::io::Read;

use crate::commands::{CommandIo, Format, IonCliCommand, WithIonCliArgument};
use crate::transcribe::{write_all_as, write_all_as_text_preserving_comments};

pub struct CatCommand;

impl IonCliCommand for CatCommand {
    fn name(&self) -> &'static str {
        "cat"
    }

    fn about(&self) -> &'static str {
        "Prints all Ion input files to the specified output in the requested format."
    }

    fn is_stable(&self) -> bool {
        true
    }

    fn is_porcelain(&self) -> bool {
        false
    }

    fn configure_args(&self, command: Command) -> Command {
        command
            .alias("dump")
            .with_input()
            .with_output()
            .with_format()
            .with_syntax_highlighting()
            .with_ion_version()
            .arg(
                Arg::new("strip-comments")
                    .long("strip-comments")
                    .action(ArgAction::SetTrue)
                    .help(
                        "Discard Ion comments instead of preserving them. By default, comments \
                         in text input are preserved in text output. Comments are always \
                         discarded when the output format is binary.",
                    ),
            )
    }

    fn run(&self, _command_path: &mut Vec<String>, args: &ArgMatches) -> Result<()> {
        let strip_comments = args
            .get_one::<bool>("strip-comments")
            .copied()
            .unwrap_or(false);
        let transform = None::<fn(Element) -> Result<Element>>;
        CommandIo::new(args)?.for_each_input(|output, input| {
            let (encoding, format) = (*output.encoding(), *output.format());
            // Ion comments exist only in text and are discarded by the value-level reader, so they
            // can only be recovered when the output is also text. Preserve them by default in that
            // case; binary output and an explicit --strip-comments both use the standard path.
            match format {
                Format::Text(text_format) if !strip_comments => {
                    let mut data = Vec::new();
                    input.into_source().read_to_end(&mut data)?;
                    write_all_as_text_preserving_comments(&data, output, encoding, text_format)?;
                }
                _ => {
                    let mut reader = Reader::new(AnyEncoding, input.into_source())?;
                    write_all_as(&mut reader, output, encoding, format, transform)?;
                }
            }
            Ok(())
        })
    }
}

use std::cmp::min;
use std::fmt::{Display, Write};
use std::fs::File;
use std::io;
use std::io::BufWriter;
use std::ops::Range;
use std::str::{from_utf8_unchecked, FromStr};

use anyhow::{bail, Context, Result};
use clap::{App, Arg, ArgMatches};
use colored::Colorize;
use ion_rs::result::IonResult;
use ion_rs::text::writer::TextWriter;
use ion_rs::{IonType, RawBinaryReader, SystemReader, SystemStreamItem};
use memmap::MmapOptions;

const ABOUT: &str =
    "A command-line processor for Ion.";

// Creates a `clap` (Command Line Arguments Parser) configuration for the `jq` command.
// This function is invoked by the `jq` command's parent, `beta`, so it can describe its
// child commands.
pub fn app() -> App<'static, 'static> {
    App::new("query")
        .about(ABOUT)
        .arg(
            Arg::with_name("jq")
                .long("jq")
                .short("j")
                .takes_value(false)
                .help("Use jq query syntax"),
        )
}

// This function is invoked by the `jq` command's parent, `beta`.
pub fn run(_command_name: &str, matches: &ArgMatches<'static>) -> Result<()> {
    todo!()
}

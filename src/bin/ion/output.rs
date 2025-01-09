use crate::file_writer::FileWriter;
use std::io::Write;
use termcolor::{ColorSpec, StandardStreamLock, WriteColor};

/// Statically dispatches writes to either an output file or STDOUT while also supporting `termcolor`
/// style escape sequences when the target is a TTY.
pub enum CommandOutput<'a> {
    StdOut(StandardStreamLock<'a>),
    File(FileWriter),
}

impl Write for CommandOutput<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        use CommandOutput::*;
        match self {
            StdOut(stdout) => stdout.write(buf),
            File(file_writer) => file_writer.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        use CommandOutput::*;
        match self {
            StdOut(stdout) => stdout.flush(),
            File(file_writer) => file_writer.flush(),
        }
    }
}

impl WriteColor for CommandOutput<'_> {
    fn supports_color(&self) -> bool {
        use CommandOutput::*;
        match self {
            StdOut(stdout) => stdout.supports_color(),
            File(file_writer) => file_writer.supports_color(),
        }
    }

    fn set_color(&mut self, spec: &ColorSpec) -> std::io::Result<()> {
        use CommandOutput::*;
        match self {
            StdOut(stdout) => stdout.set_color(spec),
            File(file_writer) => file_writer.set_color(spec),
        }
    }

    fn reset(&mut self) -> std::io::Result<()> {
        use CommandOutput::*;
        match self {
            StdOut(stdout) => stdout.reset(),
            File(file_writer) => file_writer.reset(),
        }
    }
}

use crate::file_writer::FileWriter;
use anyhow::bail;
use ion_rs::{v1_0, v1_1, Encoding, Format, IonEncoding, Writer};
use ion_rs::{IonResult, SequenceWriter, WriteAsIon};
use itertools::Itertools;
use std::io::Write;
use termcolor::{ColorSpec, StandardStreamLock, WriteColor};

/// Statically dispatches writes to either an output file or STDOUT while also supporting `termcolor`
/// style escape sequences when the target is a TTY.
pub enum CommandOutput<'a> {
    StdOut(StandardStreamLock<'a>, CommandOutputSpec),
    File(FileWriter, CommandOutputSpec),
}

pub enum CommandOutputWriter<'a, 'b> {
    Text_1_0(Writer<v1_0::Text, &'b mut CommandOutput<'a>>),
    Binary_1_0(Writer<v1_0::Binary, &'b mut CommandOutput<'a>>),
    Text_1_1(Writer<v1_1::Text, &'b mut CommandOutput<'a>>),
    Binary_1_1(Writer<v1_1::Binary, &'b mut CommandOutput<'a>>),
}

impl<'a, 'b> CommandOutputWriter<'a, 'b> {
    pub fn write<V: WriteAsIon>(&mut self, value: V) -> IonResult<&mut Self> {
        match self {
            CommandOutputWriter::Text_1_0(w) => w.write(value).map(|_| ())?,
            CommandOutputWriter::Binary_1_0(w) => w.write(value).map(|_| ())?,
            CommandOutputWriter::Text_1_1(w) => w.write(value).map(|_| ())?,
            CommandOutputWriter::Binary_1_1(w) => w.write(value).map(|_| ())?,
        }

        Ok(self)
    }

    /// Writes bytes of previously encoded values to the output stream.
    pub fn flush(&mut self) -> IonResult<()> {
        match self {
            CommandOutputWriter::Text_1_0(w) => w.flush(),
            CommandOutputWriter::Binary_1_0(w) => w.flush(),
            CommandOutputWriter::Text_1_1(w) => w.flush(),
            CommandOutputWriter::Binary_1_1(w) => w.flush(),
        }
    }

    pub fn close(self) -> IonResult<()> {
        match self {
            CommandOutputWriter::Text_1_0(w) => w.close().map(|_| ())?,
            CommandOutputWriter::Binary_1_0(w) => w.close().map(|_| ())?,
            CommandOutputWriter::Text_1_1(w) => w.close().map(|_| ())?,
            CommandOutputWriter::Binary_1_1(w) => w.close().map(|_| ())?,
        }

        Ok(())
    }
}

impl<'a> CommandOutput<'a> {
    pub fn spec(&self) -> &CommandOutputSpec {
        match self {
            CommandOutput::StdOut(_, spec) => spec,
            CommandOutput::File(_, spec) => spec,
        }
    }

    pub fn format(&self) -> &Format {
        &self.spec().format
    }

    pub fn encoding(&self) -> &IonEncoding {
        &self.spec().encoding
    }

    pub fn as_writer<'b>(&'b mut self) -> anyhow::Result<CommandOutputWriter<'a, 'b>> {
        let CommandOutputSpec { format, encoding } = *self.spec();

        Ok(match (encoding, format) {
            (IonEncoding::Text_1_0, Format::Text(text_format)) => CommandOutputWriter::Text_1_0(
                Writer::new(v1_0::Text.with_format(text_format), self)?,
            ),
            (IonEncoding::Text_1_1, Format::Text(text_format)) => CommandOutputWriter::Text_1_1(
                Writer::new(v1_1::Text.with_format(text_format), self)?,
            ),
            (IonEncoding::Binary_1_0, Format::Binary) => {
                CommandOutputWriter::Binary_1_0(Writer::new(v1_0::Binary, self)?)
            }
            (IonEncoding::Binary_1_1, Format::Binary) => {
                CommandOutputWriter::Binary_1_1(Writer::new(v1_1::Binary, self)?)
            }
            unrecognized => bail!("unsupported format '{:?}'", unrecognized),
        })
    }
}

#[derive(Debug, Copy, Clone)]
pub struct CommandOutputSpec {
    pub format: Format,
    pub encoding: IonEncoding,
}

impl Write for CommandOutput<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        use CommandOutput::*;
        match self {
            StdOut(stdout, ..) => stdout.write(buf),
            File(file_writer, ..) => file_writer.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        use CommandOutput::*;
        match self {
            StdOut(stdout, ..) => stdout.flush(),
            File(file_writer, ..) => file_writer.flush(),
        }
    }
}

impl WriteColor for CommandOutput<'_> {
    fn supports_color(&self) -> bool {
        use CommandOutput::*;
        match self {
            StdOut(stdout, ..) => stdout.supports_color(),
            File(file_writer, ..) => file_writer.supports_color(),
        }
    }

    fn set_color(&mut self, spec: &ColorSpec) -> std::io::Result<()> {
        use CommandOutput::*;
        match self {
            StdOut(stdout, ..) => stdout.set_color(spec),
            File(file_writer, ..) => file_writer.set_color(spec),
        }
    }

    fn reset(&mut self) -> std::io::Result<()> {
        use CommandOutput::*;
        match self {
            StdOut(stdout, ..) => stdout.reset(),
            File(file_writer, ..) => file_writer.reset(),
        }
    }
}

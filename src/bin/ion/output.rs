use crate::file_writer::FileWriter;
use anyhow::bail;
use ion_rs::{v1_0, v1_1, Format, IonEncoding, Writer};
use ion_rs::{IonResult, WriteAsIon};
use std::io;
use std::io::Write;
use syntect::dumps::from_uncompressed_data;
use syntect::easy::HighlightLines;
use syntect::highlighting::Style;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;
use syntect_assets::assets::HighlightingAssets;
use termcolor::{Color, ColorSpec, StandardStreamLock, WriteColor};

/// Statically dispatches writes to either an output file or STDOUT while also supporting
/// `termcolor` style escape sequences when the target is a TTY.
pub enum CommandOutput<'a> {
    HighlightedOut(HighlightedStreamWriter<'a>, CommandOutputSpec),
    StdOut(StandardStreamLock<'a>, CommandOutputSpec),
    File(FileWriter, CommandOutputSpec),
}

pub struct HighlightedStreamWriter<'a> {
    assets: HighlightingAssets,
    syntaxes: SyntaxSet,
    stdout: StandardStreamLock<'a>,
}

impl<'a> HighlightedStreamWriter<'a> {
    pub(crate) fn new(stdout: StandardStreamLock<'a>) -> Self {
        // Using syntect-assets for an increased number of supported themes
        // Perhaps ideally we'd pull in the assets folder from sharkdp/bat or something
        // An older version of that is essentially what syntect-assets is
        let assets = HighlightingAssets::from_binary();
        // Switch between .newlines and .nonewlines depending on format?
        // Only if we have to. We have a .nonewlines file in assets, but comments in syntect
        // lead me to believe that nonewlines mode is buggier and less performant.
        // Consider using include_dir here, e.g. include_dir!("$CARGO_MANIFEST_DIR/assets"),
        // especially if we decided to go the route of managing themes ourselves
        let syntaxes: SyntaxSet =
            from_uncompressed_data(include_bytes!("assets/ion.newlines.packdump"))
                .expect("Failed to load syntaxes");
        Self {
            assets,
            syntaxes,
            stdout,
        }
    }
}

#[allow(non_camel_case_types)]
pub enum CommandOutputWriter<'a, 'b> {
    Text_1_0(Writer<v1_0::Text, &'b mut CommandOutput<'a>>),
    Binary_1_0(Writer<v1_0::Binary, &'b mut CommandOutput<'a>>),
    Text_1_1(Writer<v1_1::Text, &'b mut CommandOutput<'a>>),
    Binary_1_1(Writer<v1_1::Binary, &'b mut CommandOutput<'a>>),
}

impl CommandOutputWriter<'_, '_> {
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
    #[allow(dead_code)]
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
            CommandOutput::HighlightedOut(_, spec) => spec,
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

impl Write for HighlightedStreamWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let output = std::str::from_utf8(buf).unwrap();

        let ion_syntax = &self.syntaxes.find_syntax_by_name("ion").unwrap();
        // There's a lot to learn from sharkdp/bat the subject of automated light/dark theming,
        // see src/theme.rs in: https://github.com/sharkdp/bat/pull/2896
        // Here we will hardcode something "dark" until someone complains or sends a patch
        let theme = &self.assets.get_theme("Monokai Extended"); //TODO: choose theme somehow
        let mut highlighter = HighlightLines::new(ion_syntax, theme);

        for line in LinesWithEndings::from(output) {
            let ranges: Vec<(Style, &str)> =
                highlighter.highlight_line(line, &self.syntaxes).unwrap();
            for &(ref style, text) in ranges.iter() {
                // We won't mess with the background colors
                let color = Some(Color::Rgb(
                    style.foreground.r,
                    style.foreground.g,
                    style.foreground.b,
                ));
                let mut style = ColorSpec::new();
                style.set_fg(color);
                self.stdout.set_color(&style)?;
                write!(self.stdout, "{}", text)?;
            }
        }
        // If we got here we succeeded in writing all the input bytes, so report that len
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stdout.flush()
    }
}

impl WriteColor for HighlightedStreamWriter<'_> {
    fn supports_color(&self) -> bool {
        // HighlightedStreamWriter is only used when syntect is managing the color
        false
    }

    fn set_color(&mut self, _spec: &ColorSpec) -> io::Result<()> {
        // When asked to change the color spec, do nothing.
        Ok(())
    }

    fn reset(&mut self) -> io::Result<()> {
        // When asked to reset the color spec to the default settings, do nothing.
        Ok(())
    }
}

impl Write for CommandOutput<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        use CommandOutput::*;
        match self {
            HighlightedOut(highlighted_writer, ..) => highlighted_writer.write(buf),
            StdOut(stdout, ..) => stdout.write(buf),
            File(file_writer, ..) => file_writer.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        use CommandOutput::*;
        match self {
            HighlightedOut(highlighted_writer, ..) => highlighted_writer.flush(),
            StdOut(stdout, ..) => stdout.flush(),
            File(file_writer, ..) => file_writer.flush(),
        }
    }
}

impl WriteColor for CommandOutput<'_> {
    fn supports_color(&self) -> bool {
        use CommandOutput::*;
        match self {
            HighlightedOut(highlighted_writer, ..) => highlighted_writer.supports_color(),
            StdOut(stdout, ..) => stdout.supports_color(),
            File(file_writer, ..) => file_writer.supports_color(),
        }
    }

    fn set_color(&mut self, spec: &ColorSpec) -> io::Result<()> {
        use CommandOutput::*;
        match self {
            HighlightedOut(highlighted_writer, ..) => highlighted_writer.set_color(spec),
            StdOut(stdout, ..) => stdout.set_color(spec),
            File(file_writer, ..) => file_writer.set_color(spec),
        }
    }

    fn reset(&mut self) -> io::Result<()> {
        use CommandOutput::*;
        match self {
            HighlightedOut(highlighted_writer, ..) => highlighted_writer.reset(),
            StdOut(stdout, ..) => stdout.reset(),
            File(file_writer, ..) => file_writer.reset(),
        }
    }
}

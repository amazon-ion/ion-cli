use termcolor::{ColorSpec, WriteColor};
use std::io;
use std::io::{BufWriter, Write};
use std::fs::File;

/// A buffered `io::Write` implementation that implements [`WriteColor`] by reporting that it does
/// not support TTY escape sequences and treating all requests to change or reset the current color
/// as no-ops.
//
// When writing to a file instead of a TTY, we don't want to use `termcolor` escape sequences as
// they would be stored as literal bytes rather than being interpreted. To achieve this, we need an
// `io::Write` implementation that also implements `termcolor`'s `WriteColor` trait. `WriteColor`
// allows the type to specify to whether it supports interpreting escape codes.
//
// We cannot implement `WriteColor` for `BufWriter<File>` directly due to Rust's coherence rules. Our
// crate must own the trait, the implementing type, or both. The `FileWriter` type defined below
// is a simple wrapper around a `BufWriter<File>` that implements both `io::Write` and `termcolor`'s
// `WriteColor` trait.
pub struct FileWriter {
    inner: BufWriter<File>,
}

impl FileWriter {
    pub fn new(file: File) -> Self {
        Self { inner: BufWriter::new(file) }
    }
}

// Delegates all `io::Write` methods to the nested `BufWriter`.
impl Write for FileWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl WriteColor for FileWriter {
    fn supports_color(&self) -> bool {
        // FileWriter is never used to write to a TTY, so it does not support escape codes.
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

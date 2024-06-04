use anyhow::{bail, Result};
use ion_rs::*;
use std::io::Write;

/// Constructs the appropriate writer for the given format, then writes all values from the
/// `Reader` to the new `Writer`.
pub(crate) fn write_all_as<I: IonInput>(
    reader: &mut Reader<AnyEncoding, I>,
    output: &mut impl Write,
    format: &str,
) -> Result<usize> {
    write_n_as(reader, output, format, usize::MAX)
}

/// Constructs the appropriate writer for the given format, then writes up to `count` values from the
/// `Reader` to the new `Writer`.
pub(crate) fn write_n_as<I: IonInput>(
    reader: &mut Reader<AnyEncoding, I>,
    output: &mut impl Write,
    format: &str,
    count: usize,
) -> Result<usize> {
    let written = match format {
        "pretty" => {
            let mut writer = Writer::new(v1_0::Text.with_format(TextFormat::Pretty), output)?;
            transcribe_n(reader, &mut writer, count)
        }
        "text" => {
            let mut writer = Writer::new(v1_0::Text.with_format(TextFormat::Compact), output)?;
            transcribe_n(reader, &mut writer, count)
        }
        "lines" => {
            let mut writer = Writer::new(v1_0::Text.with_format(TextFormat::Lines), output)?;
            transcribe_n(reader, &mut writer, count)
        }
        "binary" => {
            let mut writer = Writer::new(v1_0::Binary, output)?;
            transcribe_n(reader, &mut writer, count)
        }
        unrecognized => bail!("unsupported format '{unrecognized}'"),
    }?;
    Ok(written)
}

/// Writes up to `count` values from the `Reader` to the provided `Writer`.
fn transcribe_n(
    reader: &mut Reader<impl Decoder, impl IonInput>,
    writer: &mut Writer<impl Encoding, impl Write>,
    count: usize,
) -> Result<usize> {
    const FLUSH_EVERY_N: usize = 100;
    let mut values_since_flush: usize = 0;
    let mut index: usize = 0;

    while let Some(value) = reader.next()? {
        if index >= count {
            break;
        }

        writer.write(value)?;

        index += 1;
        values_since_flush += 1;
        if values_since_flush == FLUSH_EVERY_N {
            writer.flush()?;
            values_since_flush = 0;
        }
    }

    writer.flush()?;
    Ok(index)
}

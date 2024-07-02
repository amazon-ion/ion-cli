use anyhow::{bail, Result};
use ion_rs::*;
use std::io::Write;

/// Constructs the appropriate writer for the given format, then writes all values from the
/// `Reader` to the new `Writer`.
pub(crate) fn write_all_as<I: IonInput>(
    reader: &mut Reader<AnyEncoding, I>,
    output: &mut impl Write,
    encoding: IonEncoding,
    format: Format,
) -> Result<usize> {
    write_n_as(reader, output, encoding, format, usize::MAX)
}

/// Constructs the appropriate writer for the given format, then writes up to `count` values from the
/// `Reader` to the new `Writer`.
pub(crate) fn write_n_as<I: IonInput>(
    reader: &mut Reader<AnyEncoding, I>,
    output: &mut impl Write,
    encoding: IonEncoding,
    format: Format,
    count: usize,
) -> Result<usize> {
    let written = match (encoding, format) {
        (IonEncoding::Text_1_0, Format::Text(text_format)) => {
            let mut writer = Writer::new(v1_0::Text.with_format(text_format), output)?;
            transcribe_n(reader, &mut writer, count)
        }
        (IonEncoding::Text_1_1, Format::Text(text_format)) => {
            let mut writer = Writer::new(v1_1::Text.with_format(text_format), output)?;
            transcribe_n(reader, &mut writer, count)
        }
        (IonEncoding::Binary_1_0, Format::Binary) => {
            let mut writer = Writer::new(v1_0::Binary, output)?;
            transcribe_n(reader, &mut writer, count)
        }
        (IonEncoding::Binary_1_1, Format::Binary) => {
            let mut writer = Writer::new(v1_1::Binary, output)?;
            transcribe_n(reader, &mut writer, count)
        }
        unrecognized => bail!("unsupported format '{:?}'", unrecognized),
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

use anyhow::{bail, Result};
use ion_rs::*;
use std::io::Write;

/// Constructs the appropriate writer for the given format, then writes all values from the
/// `Reader` to the new `Writer`, applying an optional mapping function to each element.
pub(crate) fn write_all_as_with_mapper<I: IonInput>(
    reader: &mut Reader<AnyEncoding, I>,
    output: &mut impl Write,
    encoding: IonEncoding,
    format: Format,
    mapper: Option<fn(Element) -> Result<Element>>,
) -> Result<usize> {
    write_n_as_with_mapper(reader, output, encoding, format, usize::MAX, mapper)
}

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

/// Constructs the appropriate writer for the given format, then writes up to `count` values from the
/// `Reader` to the new `Writer`, applying an optional mapping function to each element.
pub(crate) fn write_n_as_with_mapper<I: IonInput>(
    reader: &mut Reader<AnyEncoding, I>,
    output: &mut impl Write,
    encoding: IonEncoding,
    format: Format,
    count: usize,
    mapper: Option<fn(Element) -> Result<Element>>,
) -> Result<usize> {
    let written = match (encoding, format) {
        (IonEncoding::Text_1_0, Format::Text(text_format)) => {
            let mut writer = Writer::new(v1_0::Text.with_format(text_format), output)?;
            transcribe_n_with_mapper(reader, &mut writer, count, mapper)
        }
        (IonEncoding::Text_1_1, Format::Text(text_format)) => {
            let mut writer = Writer::new(v1_1::Text.with_format(text_format), output)?;
            transcribe_n_with_mapper(reader, &mut writer, count, mapper)
        }
        (IonEncoding::Binary_1_0, Format::Binary) => {
            let mut writer = Writer::new(v1_0::Binary, output)?;
            transcribe_n_with_mapper(reader, &mut writer, count, mapper)
        }
        (IonEncoding::Binary_1_1, Format::Binary) => {
            let mut writer = Writer::new(v1_1::Binary, output)?;
            transcribe_n_with_mapper(reader, &mut writer, count, mapper)
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

    while let Some(lazy_value) = reader.next()? {
        if index >= count {
            break;
        }

        writer.write(lazy_value)?;

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

/// Writes up to `count` values from the `Reader` to the provided `Writer`,
/// applying an optional mapping function to each element.
fn transcribe_n_with_mapper(
    reader: &mut Reader<impl Decoder, impl IonInput>,
    writer: &mut Writer<impl Encoding, impl Write>,
    count: usize,
    mapper: Option<fn(Element) -> Result<Element>>,
) -> Result<usize> {
    const FLUSH_EVERY_N: usize = 100;
    let mut values_since_flush: usize = 0;
    let mut index: usize = 0;

    while let Some(lazy_value) = reader.next()? {
        if index >= count {
            break;
        }

        match mapper {
            Some(map_fn) => {
                let element = Element::try_from(lazy_value.read()?)?;
                let transformed_element = map_fn(element)?;
                writer.write(&transformed_element)?;
            }
            None => {
                writer.write(lazy_value)?;
            }
        }

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

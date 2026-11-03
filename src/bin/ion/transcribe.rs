use crate::commands::Format;
use anyhow::{bail, Result};
use ion_rs::*;
use std::io::Write;

/// Constructs the appropriate writer for the given format, then writes all values from the
/// `Reader` to the new `Writer`, applying an optional mapping function to each element.
pub(crate) fn write_all_as<I: IonInput, M: Fn(Element) -> Result<Element>>(
    reader: &mut Reader<AnyEncoding, I>,
    output: &mut impl Write,
    encoding: IonEncoding,
    format: Format,
    mapper: Option<M>,
) -> Result<usize> {
    write_n_as(reader, output, encoding, format, usize::MAX, mapper)
}

/// Constructs the appropriate writer for the given format, then writes up to `count` values from the
/// `Reader` to the new `Writer`, applying an optional mapping function to each element.
pub(crate) fn write_n_as<I: IonInput, M: Fn(Element) -> Result<Element>>(
    reader: &mut Reader<AnyEncoding, I>,
    output: &mut impl Write,
    encoding: IonEncoding,
    format: Format,
    count: usize,
    mapper: Option<M>,
) -> Result<usize> {
    let written = match (encoding, format) {
        (IonEncoding::Text_1_0, Format::Text(text_format)) => {
            let mut writer = Writer::new(v1_0::Text.with_format(text_format), output)?;
            transcribe_n(&mut writer, reader, count, mapper)
        }
        (IonEncoding::Text_1_1, Format::Text(text_format)) => {
            let mut writer = Writer::new(v1_1::Text.with_format(text_format), output)?;
            transcribe_n(&mut writer, reader, count, mapper)
        }
        (IonEncoding::Binary_1_0, Format::Binary) => {
            let mut writer = Writer::new(v1_0::Binary, output)?;
            transcribe_n(&mut writer, reader, count, mapper)
        }
        (IonEncoding::Binary_1_1, Format::Binary) => {
            let mut writer = Writer::new(v1_1::Binary, output)?;
            transcribe_n(&mut writer, reader, count, mapper)
        }
        unrecognized => bail!("unsupported format '{:?}'", unrecognized),
    }?;
    Ok(written)
}

/// Transcribes all values from `data` to `output` in the requested text `encoding`/`format`,
/// preserving any Ion comments that appear between top-level values in the input.
///
/// Ion comments are not part of the Ion data model, so the value-level reader discards them.
/// To preserve them, this function reads the input into memory and uses each value's source byte
/// range (via the reader's tooling APIs) to locate the "trivia" regions that sit *between*
/// top-level values. Per the Ion grammar, those regions contain only whitespace and comments, so
/// the comments can be recovered unambiguously and re-emitted around the pretty-printed values.
///
/// Only text output is supported; binary Ion has no notion of comments. Comments that appear
/// *inside* a top-level value (for example, between a container's child values or between an
/// annotation and its value) fall within that value's source range and are not preserved, because
/// the value itself is re-serialized by the writer.
pub(crate) fn write_all_as_text_preserving_comments(
    data: &[u8],
    output: &mut impl Write,
    encoding: IonEncoding,
    text_format: TextFormat,
) -> Result<usize> {
    match encoding {
        IonEncoding::Text_1_0 => {
            let writer = Writer::new(v1_0::Text.with_format(text_format), output)?;
            transcribe_preserving_comments(writer, data)
        }
        IonEncoding::Text_1_1 => {
            let writer = Writer::new(v1_1::Text.with_format(text_format), output)?;
            transcribe_preserving_comments(writer, data)
        }
        unsupported => bail!(
            "comment preservation is only supported for text output, but got '{:?}'",
            unsupported
        ),
    }
}

/// Shared implementation of [`write_all_as_text_preserving_comments`] that is generic over the
/// concrete text writer encoding.
fn transcribe_preserving_comments<E: Encoding, W: Write>(
    mut writer: Writer<E, W>,
    data: &[u8],
) -> Result<usize> {
    let mut reader = Reader::new(AnyEncoding, data)?;
    // Byte offset into `data` of the first byte that has not yet been accounted for by an emitted
    // value or comment.
    let mut cursor: usize = 0;
    let mut index: usize = 0;
    // Comment recovery relies on each value exposing its source byte range. Macro-expanded values
    // (Ion 1.1) do not have a single source range; if we encounter one, we stop recovering comments
    // to avoid re-scanning already-consumed input, but continue transcribing values faithfully.
    let mut tracking_comments = true;

    while let Some(value) = reader.next()? {
        if tracking_comments {
            match value.expanded().range() {
                Some(range) if range.start >= cursor => {
                    // Flush buffered value bytes so the comments we write next land *after* them.
                    writer.flush()?;
                    write_comments_in(writer.output_mut(), &data[cursor..range.start])?;
                    cursor = range.end;
                }
                // A range we can't use for ordering (out of order or absent); stop recovering.
                _ => tracking_comments = false,
            }
        }
        writer.write(value)?;
        index += 1;
    }

    if tracking_comments && cursor < data.len() {
        // Preserve any trailing comments that follow the last value.
        writer.flush()?;
        write_comments_in(writer.output_mut(), &data[cursor..])?;
    }

    writer.flush()?;
    Ok(index)
}

/// Scans `region` -- a slice of input that contains only whitespace and Ion comments -- and writes
/// each comment to `output`, one per line. Both line (`//`) and block (`/* */`) comments are
/// recognized. Any non-comment bytes (whitespace, and defensively any other content) are skipped.
fn write_comments_in(output: &mut impl Write, region: &[u8]) -> Result<()> {
    let mut i = 0;
    while i < region.len() {
        if region[i] == b'/' && i + 1 < region.len() && region[i + 1] == b'/' {
            // Line comment: runs until the next line break or the end of the region.
            let start = i;
            i += 2;
            while i < region.len() && region[i] != b'\n' && region[i] != b'\r' {
                i += 1;
            }
            write_comment_line(output, &region[start..i])?;
        } else if region[i] == b'/' && i + 1 < region.len() && region[i + 1] == b'*' {
            // Block comment: runs until the closing `*/` (or the end of the region if unterminated,
            // though a well-formed input will always close it).
            let start = i;
            i += 2;
            while i + 1 < region.len() && !(region[i] == b'*' && region[i + 1] == b'/') {
                i += 1;
            }
            let end = (i + 2).min(region.len());
            i = end;
            write_comment_line(output, &region[start..end])?;
        } else {
            i += 1;
        }
    }
    Ok(())
}

/// Writes a single comment on its own line.
fn write_comment_line(output: &mut impl Write, comment: &[u8]) -> Result<()> {
    output.write_all(comment)?;
    output.write_all(b"\n")?;
    Ok(())
}

/// Writes up to `count` values from the `Reader` to the provided `Writer`,
/// applying an optional mapping function to each element.
fn transcribe_n<M: Fn(Element) -> Result<Element>>(
    writer: &mut Writer<impl Encoding, impl Write>,
    reader: &mut Reader<impl Decoder, impl IonInput>,
    count: usize,
    mapper: Option<M>,
) -> Result<usize> {
    const FLUSH_EVERY_N: usize = 100;
    let mut values_since_flush: usize = 0;
    let mut index: usize = 0;

    while let Some(lazy_value) = reader.next()? {
        if index >= count {
            break;
        }

        match mapper {
            Some(ref map_fn) => {
                // materialize and apply mapper
                let element = Element::try_from(lazy_value.read()?)?;
                let transformed = map_fn(element)?;
                writer.write(&transformed)?;
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

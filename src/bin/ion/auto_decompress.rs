use infer::Type;
use std::io;
use std::io::{BufReader, Cursor, Read};

use crate::input::CompressionDetected;
use ion_rs::IonResult;

/// Auto-detects a compressed byte stream and wraps the original reader
/// into a reader that transparently decompresses.
pub type AutoDecompressingReader = BufReader<Box<dyn Read>>;

pub fn decompress<R>(
    mut reader: R,
    header_len: usize,
) -> IonResult<(CompressionDetected, AutoDecompressingReader)>
where
    R: Read + 'static,
{
    // read header
    let mut header_bytes = vec![0; header_len];
    let nread = read_reliably(&mut reader, &mut header_bytes)?;
    header_bytes.truncate(nread);

    let detected_type = infer::get(&header_bytes);
    let header = Cursor::new(header_bytes);
    let stream = header.chain(reader);

    // detect compression type and wrap reader in a decompressor
    match detected_type.as_ref().map(Type::extension) {
        Some("gz") => {
            // "rewind" to let the decompressor read magic bytes again
            let zreader = Box::new(flate2::read::MultiGzDecoder::new(stream));
            Ok((CompressionDetected::Gzip, BufReader::new(zreader)))
        }
        Some("zst") => {
            let zreader = Box::new(zstd::stream::read::Decoder::new(stream)?);
            Ok((CompressionDetected::Zstd, BufReader::new(zreader)))
        }
        _ => Ok((CompressionDetected::None, BufReader::new(Box::new(stream)))),
    }
}

/// Similar to [`Read::read()`], but loops in case of fragmented reads.
pub fn read_reliably<R: Read>(reader: &mut R, buf: &mut [u8]) -> io::Result<usize> {
    let mut nread = 0;
    while nread < buf.len() {
        match reader.read(&mut buf[nread..]) {
            Ok(0) => break,
            Ok(n) => nread += n,
            Err(e) => return Err(e),
        }
    }
    Ok(nread)
}

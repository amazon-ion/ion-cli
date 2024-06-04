use ion_rs::IonResult;
use std::io;
use std::io::{BufRead, BufReader, Chain, Cursor, Read};

/// Auto-detects a compressed byte stream and wraps the original reader
/// into a reader that transparently decompresses.
///
// To support non-seekable readers like `Stdin`, we could have used a
// full-blown buffering wrapper with unlimited rewinds, but since we only
// need the first few magic bytes at offset 0, we cheat and instead make a
// `Chain` reader from the buffered header followed by the original reader.
//
// The choice of `Chain` type here is not quite necessary: it could have
// been simply `dyn BufRead`, but there is no `ToIonDataSource` trait
// implementation for `dyn BufRead` at the moment.
type AutoDecompressingReader = Chain<Box<dyn BufRead>, Box<dyn BufRead>>;

pub fn auto_decompressing_reader<R>(
    mut reader: R,
    header_len: usize,
) -> IonResult<AutoDecompressingReader>
where
    R: BufRead + 'static,
{
    // read header
    let mut header_bytes = vec![0; header_len];
    let nread = read_reliably(&mut reader, &mut header_bytes)?;
    header_bytes.truncate(nread);

    // detect compression type and wrap reader in a decompressor
    match infer::get(&header_bytes) {
        Some(t) => match t.extension() {
            "gz" => {
                // "rewind" to let the decompressor read magic bytes again
                let header: Box<dyn BufRead> = Box::new(Cursor::new(header_bytes));
                let chain = header.chain(reader);
                let zreader = Box::new(BufReader::new(flate2::read::GzDecoder::new(chain)));
                // must return a `Chain`, so prepend an empty buffer
                let nothing: Box<dyn BufRead> = Box::new(Cursor::new(&[] as &[u8]));
                Ok(nothing.chain(zreader))
            }
            "zst" => {
                let header: Box<dyn BufRead> = Box::new(Cursor::new(header_bytes));
                let chain = header.chain(reader);
                let zreader = Box::new(BufReader::new(zstd::stream::read::Decoder::new(chain)?));
                let nothing: Box<dyn BufRead> = Box::new(Cursor::new(&[] as &[u8]));
                Ok(nothing.chain(zreader))
            }
            _ => {
                let header: Box<dyn BufRead> = Box::new(Cursor::new(header_bytes));
                Ok(header.chain(Box::new(reader)))
            }
        },
        None => {
            let header: Box<dyn BufRead> = Box::new(Cursor::new(header_bytes));
            Ok(header.chain(Box::new(reader)))
        }
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

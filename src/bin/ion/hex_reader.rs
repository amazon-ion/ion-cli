use std::io::{Bytes, Cursor, ErrorKind, Read};

/// Wraps an existing reader in order to reinterpret the content of that reader as a
/// hexadecimal-encoded byte stream.
///
/// This will silently ignore all whitespace (to allow spacing/formatting in the input).
///
/// If the input contains any characters that are not hex digits or whitespace, the `read` function
/// will (upon encountering that character) return `Err`. If the input contains an odd number of hex
/// digits, the final call to `read` will return `Err`.
pub struct HexReader<R: Read> {
    inner: Bytes<R>,
    digit_buffer: String,
}

impl<R: Read> From<R> for HexReader<R> {
    fn from(value: R) -> Self {
        Self {
            inner: value.bytes(),
            digit_buffer: String::new(),
        }
    }
}

impl<R: Read> Read for HexReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.len() == 0 {
            return Ok(0);
        }

        let mut bytes_read = 0usize;

        while let Some(b) = self.inner.next() {
            let c = char::from(b?);
            if c.is_digit(16) {
                self.digit_buffer.push(c)
            } else if c.is_whitespace() {
                // Ignore these characters
            } else {
                return Err(std::io::Error::new(
                    ErrorKind::InvalidData,
                    format!("not a valid hexadecimal digit: '{c}'"),
                ));
            }
            if self.digit_buffer.len() == 2 {
                // Unwrap is guaranteed not to panic because we've been putting only valid hex
                // digit characters in the `digit_buffer` String.
                buf[bytes_read] = u8::from_str_radix(self.digit_buffer.as_str(), 16).unwrap();
                bytes_read += 1;
                self.digit_buffer.clear();

                if bytes_read == buf.len() {
                    break;
                }
            }
        }
        if bytes_read == 0 && self.digit_buffer.len() > 0 {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                "found an odd number of hex digits",
            ));
        }

        Ok(bytes_read)
    }
}

#[test]
fn test_read_hex_digits() {
    let hex = "00010203";
    let reader = HexReader::from(Cursor::new(hex));
    let translated_bytes: std::io::Result<Vec<_>> = reader.bytes().collect();
    let expected = vec![0u8, 1, 2, 3];
    assert_eq!(expected, translated_bytes.unwrap())
}

#[test]
fn test_read_hex_digits_with_whitespace() {
    let hex = "00   01\n  02 \t \t\t  03 \r\n04";
    let reader = HexReader::from(Cursor::new(hex));
    let translated_bytes: std::io::Result<Vec<_>> = reader.bytes().collect();
    let expected = vec![0u8, 1, 2, 3, 4];
    assert_eq!(expected, translated_bytes.unwrap())
}

#[test]
fn test_hex_reader_correctly_handles_composed_readers() {
    let hex0 = "00 01 0  ";
    let hex1 = "      ";
    let hex2 = " 2 03";
    let hex = Cursor::new(hex0)
        .chain(Cursor::new(hex1))
        .chain(Cursor::new(hex2));
    let reader = HexReader::from(hex);
    let translated_bytes: std::io::Result<Vec<_>> = reader.bytes().collect();
    let expected = vec![0u8, 1, 2, 3];
    assert_eq!(expected, translated_bytes.unwrap())
}

#[test]
fn test_read_odd_number_of_hex_digits() {
    let hex = "000102030";
    let reader = HexReader::from(Cursor::new(hex));
    let translated_bytes: std::io::Result<Vec<_>> = reader.bytes().collect();
    assert!(translated_bytes.is_err())
}

#[test]
fn test_read_hex_digits_with_invalid_char() {
    let hex = "000102030Q";
    let reader = HexReader::from(Cursor::new(hex));
    let translated_bytes: std::io::Result<Vec<_>> = reader.bytes().collect();
    assert!(translated_bytes.is_err())
}

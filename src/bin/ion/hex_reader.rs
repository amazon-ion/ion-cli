use ion_rs::{IonInput, IonStream};
use std::io::{Bytes, Error, ErrorKind, Read};

/// Wraps an existing reader in order to reinterpret the content of that reader as a
/// hexadecimal-encoded byte stream.
///
/// This can read hex digit pairs in the form `0xHH` or `HH` where `H` is a case-insensitive
/// hexadecimal digit. Between pairs, there can be any number of whitespace characters or commas.
/// These are the only accepted characters.
///
/// If the input contains any unacceptable characters or unpaired hex digits, the `read` function
/// will (upon encountering that character) return `Err`.
pub struct HexReader<R: Read> {
    inner: Bytes<R>,
    digit_state: DigitState,
}

#[derive(Eq, PartialEq, Debug)]
enum DigitState {
    /// The reader is ready to encounter a hexadecimal-encoded byte.
    Empty,
    /// The reader has encountered a `0`. This is an ambiguous state where we could be looking at a
    /// `0` that is the first in a pair with another hex digit, or it could be the `0` before an `x`.
    /// In other words, we're at the start of `0H` or `0xHH`, and we don't yet know which it is.
    Zero,
    /// The reader has seen `0x`. The next character must be a hex digit, which is the upper nibble
    /// of the hex-encoded byte.
    ZeroX,
    /// The reader has seen either `0xH` or `H`. The next character must be a hex digit, and will
    /// form a complete hex-encoded byte.
    HasUpperNibble(char),
}

impl<R: Read> IonInput for HexReader<R> {
    type DataSource = IonStream<Self>;

    fn into_data_source(self) -> Self::DataSource {
        IonStream::new(self)
    }
}

impl<R: Read> From<R> for HexReader<R> {
    fn from(value: R) -> Self {
        Self {
            inner: value.bytes(),
            digit_state: DigitState::Empty,
        }
    }
}

impl<R: Read> Read for HexReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        let mut bytes_read = 0usize;

        for byte in &mut self.inner {
            let c = char::from(byte?);

            use DigitState::*;
            match self.digit_state {
                Empty if c.is_whitespace() || c == ',' => { /* Ignore these characters */ }
                // We've encountered either the first digit or the `0` of `0x`.
                Empty if c == '0' => self.digit_state = Zero,
                // Now we know that this hex-encoded byte is going to be `0xHH` rather than `0H`
                Zero if c == 'x' => self.digit_state = ZeroX,
                // Reading the first digit of the hex-encoded byte
                Empty | ZeroX if c.is_ascii_hexdigit() => self.digit_state = HasUpperNibble(c),
                // Reading the second digit of the hex-encoded byte
                Zero if c.is_ascii_hexdigit() => {
                    // Unwrap is guaranteed not to panic because we've been putting only valid hex
                    // digit characters in the `digit_buffer` String.
                    let value = c.to_digit(16).unwrap();
                    // This unwrap is guaranteed not to panic because the max it could be is 0x0F
                    buf[bytes_read] = u8::try_from(value).unwrap();
                    bytes_read += 1;
                    self.digit_state = Empty;
                }
                HasUpperNibble(c0) if c.is_ascii_hexdigit() => {
                    // The first unwrap is guaranteed not to panic because we already know that both
                    // chars are valid hex digits.
                    // The second unwrap is guaranteed not to panic because the max it could be is 0x0F
                    let high_nibble: u8 = c0.to_digit(16).unwrap().try_into().unwrap();
                    let low_nibble: u8 = c.to_digit(16).unwrap().try_into().unwrap();
                    buf[bytes_read] = (high_nibble << 4) + low_nibble;
                    bytes_read += 1;
                    self.digit_state = Empty;
                }
                // Error cases
                _ if c.is_whitespace() => {
                    return Err(Error::new(
                        ErrorKind::InvalidData,
                        format!("unexpected whitespace when digit expected: '{c}'"),
                    ))
                }
                _ => {
                    return Err(Error::new(
                        ErrorKind::InvalidData,
                        format!("not a valid hexadecimal digit: '{c}'"),
                    ))
                }
            }

            if bytes_read == buf.len() {
                break;
            }
        }

        if bytes_read < buf.len() && self.digit_state != DigitState::Empty {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "found an odd number of hex digits",
            ));
        }

        Ok(bytes_read)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

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
    fn test_read_hex_digits_with_leading_0x() {
        let hex = "0x00 0x01 0x02 0x03 0x04";
        let reader = HexReader::from(Cursor::new(hex));
        let translated_bytes: std::io::Result<Vec<_>> = reader.bytes().collect();
        let expected = vec![0u8, 1, 2, 3, 4];
        assert_eq!(expected, translated_bytes.unwrap())
    }

    #[test]
    fn test_read_hex_digits_with_commas() {
        let hex = "00,01,02,03,04";
        let reader = HexReader::from(Cursor::new(hex));
        let translated_bytes: std::io::Result<Vec<_>> = reader.bytes().collect();
        let expected = vec![0u8, 1, 2, 3, 4];
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
}

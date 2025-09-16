use anyhow::Result;
use ion_rs::{Element, IonType};

use super::structural_recursion::{map_structure, ElementMapper};

struct TimestampConverter;

impl ElementMapper for TimestampConverter {
    fn map(&self, element: Element) -> Result<Element> {
        Ok(match element.ion_type() {
            IonType::String => {
                let s = element.as_string().unwrap();
                if is_timestamp_like(s) {
                    if let Ok(timestamp_element) = Element::read_one(s.as_bytes()) {
                        if timestamp_element.ion_type() == IonType::Timestamp {
                            return Ok(timestamp_element);
                        }
                    }
                }
                element
            }
            _ => element,
        })
    }
}

/// Converts timestamp-like strings to Ion timestamps using iterative traversal
pub fn convert_timestamps(element: Element) -> Result<Element> {
    map_structure(element, &TimestampConverter)
}

/// Heuristic to identify strings that could be Ion timestamps
///
/// Ion timestamps follow ISO 8601 format with these constraints:
/// - Years 0001-9999 (4 digits)
/// - Precision up to nanoseconds
/// - Must have date component (YYYY, YYYY-MM, or YYYY-MM-DD)
///
/// This function uses position-based checks, which are cheaper compared to string operations:
/// - Length bounds (4-35 chars for timestamp range)
/// - Direct character position checks
fn is_timestamp_like(s: &str) -> bool {
    let len = s.len();

    // Bounds check, timestamps are 4-35 chars
    if !(4..=35).contains(&len) {
        return false;
    }

    // Must start with 4 digits
    let bytes = s.as_bytes();
    if !bytes[0].is_ascii_digit()
        || !bytes[1].is_ascii_digit()
        || !bytes[2].is_ascii_digit()
        || !bytes[3].is_ascii_digit()
    {
        return false;
    }

    match len {
        4 => false,
        5..=9 => bytes[len - 1] == b'T',
        10 => bytes[4] == b'-' && bytes[7] == b'-',
        _ => len > 10 && bytes[10] == b'T',
    }
}

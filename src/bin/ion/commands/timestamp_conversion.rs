use anyhow::Result;
use ion_rs::{Element, IonType};

use super::structural_recursion::{map_structure, ElementMapper};

struct TimestampConverter;

impl ElementMapper for TimestampConverter {
    fn map(&self, element: Element) -> Result<Element> {
        Ok(element.as_text()
            .and_then(as_timestamp)
            .unwrap_or(element))
    }
}

/// Converts timestamp-like strings to Ion timestamps using iterative traversal
pub fn convert_timestamps(element: Element) -> Result<Element> {
    map_structure(element, &TimestampConverter)
}

/// Heuristic to identify strings that could be Ion timestamps
///
/// Ion timestamps follow W3C date/time format with these constraints:
/// - Must have date component (YYYY, YYYY-MM, or YYYY-MM-DD)
/// - Precision up to fractional seconds (unlimited precision)
/// - Must end with 'T' if time components are present
/// - Time zone offset required for timestamps with time, not allowed for date-only values
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

fn as_timestamp(s: &str) -> Option<Element> {
    if !is_timestamp_like(s) {
        return None;
    }
    Element::read_one(s.as_bytes()).ok()
        .filter(|e| e.ion_type() == IonType::Timestamp)
}

use anyhow::Result;
use ion_rs::{Element, IonType};

/// Recursively converts timestamp-like strings to Ion timestamps in the given element.
pub fn convert_timestamps(element: Element) -> Result<Element> {
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
        IonType::List => {
            let list = element.as_sequence().unwrap();
            let converted: Result<Vec<_>> = list
                .elements()
                .map(|e| convert_timestamps(e.clone()))
                .collect();
            Element::from(ion_rs::List::from(converted?))
        }
        IonType::Struct => {
            let struct_val = element.as_struct().unwrap();
            let mut struct_builder = ion_rs::Struct::builder();
            for (field, value) in struct_val.fields() {
                struct_builder =
                    struct_builder.with_field(field, convert_timestamps(value.clone())?);
            }
            Element::from(struct_builder.build())
        }
        _ => element,
    })
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

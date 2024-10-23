//! Ansi Codes are a convenient way to add styling to text in a terminal.
//! There are libraries that can accomplish the same thing, but when you want to have a large block
//! of static text, sometimes it's simpler to just use `format!()` and include named substitutions
//! (like `{BOLD}`) to turn styling on and off.

// TODO: Add more constants as needed.

pub(crate) const NO_STYLE: &str = "\x1B[0m";
pub(crate) const BOLD: &str = "\x1B[1m";
pub(crate) const ITALIC: &str = "\x1B[3m";
pub(crate) const UNDERLINE: &str = "\x1B[4m";

pub(crate) const RED: &str = "\x1B[0;31m";
pub(crate) const GREEN: &str = "\x1B[0;32m";

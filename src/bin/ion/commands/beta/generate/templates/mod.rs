// This module includes constants that can be used to render templates for generating code.
// Currently, there is no other way to add resources like `.templ` files in cargo binary crate.
// Using these constants allows the binary to access templates through these constants.

/// Represents java template constants
pub(crate) mod java {
    pub(crate) const CLASS: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/bin/ion/commands/beta/generate/templates/java/class.templ"
    ));
    pub(crate) const SCALAR: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/bin/ion/commands/beta/generate/templates/java/scalar.templ"
    ));
    pub(crate) const SEQUENCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/bin/ion/commands/beta/generate/templates/java/sequence.templ"
    ));
    pub(crate) const UTIL_MACROS: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/bin/ion/commands/beta/generate/templates/java/util_macros.templ"
    ));
    pub(crate) const NESTED_TYPE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/bin/ion/commands/beta/generate/templates/java/nested_type.templ"
    ));
}

/// Represents rust template constants
pub(crate) mod rust {
    pub(crate) const STRUCT: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/bin/ion/commands/beta/generate/templates/rust/struct.templ"
    ));
    pub(crate) const SCALAR: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/bin/ion/commands/beta/generate/templates/rust/scalar.templ"
    ));
    pub(crate) const SEQUENCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/bin/ion/commands/beta/generate/templates/rust/sequence.templ"
    ));
    pub(crate) const UTIL_MACROS: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/bin/ion/commands/beta/generate/templates/rust/util_macros.templ"
    ));
    pub(crate) const RESULT: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/bin/ion/commands/beta/generate/templates/rust/result.templ"
    ));
    pub(crate) const NESTED_TYPE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/bin/ion/commands/beta/generate/templates/rust/nested_type.templ"
    ));
    pub(crate) const IMPORT: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/bin/ion/commands/beta/generate/templates/rust/import.templ"
    ));
}

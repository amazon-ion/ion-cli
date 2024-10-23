// This module includes constants that can be used to render templates for generating code.
// Currently, there is no other way to add resources like `.templ` files in cargo binary crate.
// Using these constants allows the binary to access templates through these constants.

macro_rules! include_template {
    ($file:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/bin/ion/commands/generate/templates/",
            $file
        ))
    };
}

/// Represents java template constants
pub(crate) mod java {
    pub(crate) const CLASS: &str = include_template!("java/class.templ");
    pub(crate) const SCALAR: &str = include_template!("java/scalar.templ");
    pub(crate) const SEQUENCE: &str = include_template!("java/sequence.templ");
    pub(crate) const ENUM: &str = include_template!("java/enum.templ");
    pub(crate) const UTIL_MACROS: &str = include_template!("java/util_macros.templ");
    pub(crate) const NESTED_TYPE: &str = include_template!("java/nested_type.templ");
}

/// Represents rust template constants
pub(crate) mod rust {
    pub(crate) const STRUCT: &str = include_template!("rust/struct.templ");
    pub(crate) const SCALAR: &str = include_template!("rust/scalar.templ");
    pub(crate) const SEQUENCE: &str = include_template!("rust/sequence.templ");
    pub(crate) const ENUM: &str = include_template!("rust/enum.templ");
    pub(crate) const UTIL_MACROS: &str = include_template!("rust/util_macros.templ");
    pub(crate) const RESULT: &str = include_template!("rust/result.templ");
    pub(crate) const NESTED_TYPE: &str = include_template!("rust/nested_type.templ");
    pub(crate) const IMPORT: &str = include_template!("rust/import.templ");
}

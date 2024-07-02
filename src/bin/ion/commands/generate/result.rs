use ion_schema::result::IonSchemaError;
use thiserror::Error;

/// Represents code generation result
pub type CodeGenResult<T> = Result<T, CodeGenError>;

/// Represents an error found during code generation
#[derive(Debug, Error)]
pub enum CodeGenError {
    #[error("{source:?}")]
    IonSchemaError {
        #[from]
        source: IonSchemaError,
    },
    #[error("{source:?}")]
    IoError {
        #[from]
        source: std::io::Error,
    },
    #[error("{source:?}")]
    TeraError {
        #[from]
        source: tera::Error,
    },
    #[error("{description}")]
    InvalidDataModel { description: String },
}

/// A convenience method for creating an CodeGen containing an CodeGenError::InvalidDataModel
/// with the provided description text.
pub fn invalid_abstract_data_type_error<T, S: AsRef<str>>(description: S) -> CodeGenResult<T> {
    Err(CodeGenError::InvalidDataModel {
        description: description.as_ref().to_string(),
    })
}

use crate::commands::generate::model::{
    EnumBuilderError, ScalarBuilderError, SequenceBuilderError, StructureBuilderError,
    WrappedScalarBuilderError, WrappedSequenceBuilderError,
};
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
    #[error("{description}")]
    DataModelBuilderError { description: String },
}

/// A convenience method for creating an CodeGen containing an CodeGenError::InvalidDataModel
/// with the provided description text.
pub fn invalid_abstract_data_type_error<T, S: AsRef<str>>(description: S) -> CodeGenResult<T> {
    Err(CodeGenError::InvalidDataModel {
        description: description.as_ref().to_string(),
    })
}

/// A convenience method for creating an CodeGenError::InvalidDataModel
/// with the provided description text.
pub fn invalid_abstract_data_type_raw_error<S: AsRef<str>>(description: S) -> CodeGenError {
    CodeGenError::InvalidDataModel {
        description: description.as_ref().to_string(),
    }
}

impl From<WrappedScalarBuilderError> for CodeGenError {
    fn from(value: WrappedScalarBuilderError) -> Self {
        CodeGenError::DataModelBuilderError {
            description: value.to_string(),
        }
    }
}

impl From<ScalarBuilderError> for CodeGenError {
    fn from(value: ScalarBuilderError) -> Self {
        CodeGenError::DataModelBuilderError {
            description: value.to_string(),
        }
    }
}

impl From<SequenceBuilderError> for CodeGenError {
    fn from(value: SequenceBuilderError) -> Self {
        CodeGenError::DataModelBuilderError {
            description: value.to_string(),
        }
    }
}

impl From<WrappedSequenceBuilderError> for CodeGenError {
    fn from(value: WrappedSequenceBuilderError) -> Self {
        CodeGenError::DataModelBuilderError {
            description: value.to_string(),
        }
    }
}

impl From<StructureBuilderError> for CodeGenError {
    fn from(value: StructureBuilderError) -> Self {
        CodeGenError::DataModelBuilderError {
            description: value.to_string(),
        }
    }
}

impl From<EnumBuilderError> for CodeGenError {
    fn from(value: EnumBuilderError) -> Self {
        CodeGenError::DataModelBuilderError {
            description: value.to_string(),
        }
    }
}

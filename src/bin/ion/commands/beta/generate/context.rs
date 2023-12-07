use serde::Serialize;
use std::fmt::{Display, Formatter};

/// Represents a context that will be used for code generation
pub struct CodeGenContext {
    // Initially the data_model field is set to None.
    // Once an ISL type definition is mapped to a data model this will have Some value.
    pub(crate) data_model: Option<DataModel>,
}

impl CodeGenContext {
    pub fn new() -> Self {
        Self { data_model: None }
    }

    pub fn with_data_model(&mut self, data_model: DataModel) {
        self.data_model = Some(data_model);
    }
}

/// Represents a data model type that can be used to determine which templates can be used for code generation.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum DataModel {
    Value, // a struct with a scalar value (used for `type` constraint)
    // TODO: Make Sequence parameterized over data type.
    //  add a data type for sequence here that can be used to read elements for that data type.
    Sequence, // a struct with a sequence/collection value (used for `element` constraint)
    Struct,
}

impl Display for DataModel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                DataModel::Value => "single value struct",
                DataModel::Sequence => "sequence value struct",
                DataModel::Struct => "struct",
            }
        )
    }
}

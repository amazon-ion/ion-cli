use crate::commands::beta::generate::utils::Language;
use serde::Serialize;
use tera::Tera;

/// Represents a context that will be used for code generation
pub struct CodeGenContext {
    // Represents the templating engine - tera
    pub(crate) tera: Tera,
    pub(crate) language: Language,
    // Initially the data_model field is set to None.
    // Once an ISL type definition is mapped to a data model this will have Some value.
    pub(crate) data_model: Option<DataModel>,
    // Represents a counter for naming anonymous type definitions
    pub(crate) anonymous_type_counter: usize,
}

impl CodeGenContext {
    pub fn new(language: Language) -> Self {
        Self {
            language,
            data_model: None,
            anonymous_type_counter: 0,
            tera: Tera::new("src/bin/ion/commands/beta/generate/templates/**/*.templ").unwrap(),
        }
    }

    pub fn with_data_model(&mut self, data_model: DataModel) {
        self.data_model = Some(data_model);
    }

    pub fn with_initial_data_model(&mut self) {
        // Initially the data model is set to None, this will be set with Some(_) value when data model is determined in code generation process
        self.data_model = None;
    }

    /// Returns a string that represent the template name based on data model type.
    pub fn template_name(&self) -> &str {
        if let Some(data_model) = &self.data_model {
            return match (&self.language, data_model) {
                (
                    Language::Rust,
                    DataModel::Struct | DataModel::UnitStruct | DataModel::SequenceStruct,
                ) => "struct",
                (
                    Language::Java,
                    DataModel::Struct | DataModel::UnitStruct | DataModel::SequenceStruct,
                ) => "class",
            };
        }
        "" // Default value is an empty string
    }
}

/// Represents a data model type that can be used to determine which templates can be used for code generation.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum DataModel {
    UnitStruct,     // a struct with a scalar value (used for `type` constraint)
    SequenceStruct, // a struct with a sequence value (used for `element` constraint)
    Struct,
}

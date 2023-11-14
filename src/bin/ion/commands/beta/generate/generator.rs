use crate::commands::beta::generate::context::{CodeGenContext, DataModel};
use crate::commands::beta::generate::result::{invalid_data_model_error, CodeGenResult};
use crate::commands::beta::generate::utils::{Field, Import, Language};
use convert_case::{Case, Casing};
use ion_schema::isl::isl_constraint::{IslConstraint, IslConstraintValue};
use ion_schema::isl::isl_type::IslType;
use ion_schema::isl::isl_type_reference::IslTypeRef;
use ion_schema::isl::IslSchema;
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use tera::Context;

pub(crate) struct CodeGenerator;

impl CodeGenerator {
    /// Maps the given type name to a native type based on programming language
    pub fn map_to_base_type(name: &str, language: &Language) -> String {
        match (name, language) {
            ("int", Language::Java) => "int".to_string(),
            ("string" | "symbol" | "text", _) => "String".to_string(),
            ("int", Language::Rust) => "i64".to_string(),
            ("float", Language::Rust) => "f64".to_string(),
            ("float", Language::Java) => "float".to_string(),
            ("bool", Language::Rust) => "bool".to_string(),
            ("bool", Language::Java) => "boolean".to_string(),
            ("blob" | "clob" | "lob", Language::Rust) => "Vec<u8>".to_string(),
            ("blob" | "clob" | "lob", Language::Java) => "byte[]".to_string(),
            ("decimal" | "timestamp" | "number", _) => {
                unimplemented!("Decimal, Number and Timestamp aren't support yet!")
            }
            ("list" | "struct" | "sexp" | "document", _) => {
                unimplemented!("Generic containers aren't supported yet!")
            }
            (_, _) => name.to_case(Case::UpperCamel),
        }
    }

    /// Returns true if its a built in type otherwise returns false
    pub fn is_built_in_type(name: &str) -> bool {
        matches!(
            name,
            "int" | "i64" | "String" | "bool" | "boolean" | "byte[]" | "Vec<u8>" | "float" | "f64"
        )
    }

    /// Represents a tera filter that converts given tera string value to upper camel case
    /// Returns error if the given value is not a string
    /// For more information: https://docs.rs/tera/1.19.0/tera/struct.Tera.html#method.register_filter
    pub fn upper_camel(
        value: &tera::Value,
        _map: &HashMap<String, tera::Value>,
    ) -> Result<tera::Value, tera::Error> {
        Ok(tera::Value::String(
            value
                .as_str()
                .ok_or(tera::Error::msg("Required string for this filter"))?
                .to_case(Case::UpperCamel),
        ))
    }

    /// Generates code for given Ion Schema
    pub fn generate_code(
        language: Language,
        schema: IslSchema,
        output: &Path,
    ) -> CodeGenResult<()> {
        // this will be used for Rust to create mod.rs which lists all the generates modules
        let mut modules = vec![];
        let mut module_context = tera::Context::new();
        let mut code_gen_context = CodeGenContext::new(language);

        // Register a tera filter that can be used to convert a string to upper camel case
        code_gen_context
            .tera
            .register_filter("upper_camel", Self::upper_camel);

        // Create a module for storing all the generated code in the output directory
        fs::create_dir(output.join("ion_data_model"))?;

        for isl_type in schema.types() {
            // Initialize the data model as None for each type
            // this will be filled with appropriate data model when code is generated for the type
            code_gen_context.data_model = None;
            Self::generate_data_model(output, &mut modules, &mut code_gen_context, isl_type)?;
        }

        if code_gen_context.language == Language::Rust {
            module_context.insert("modules", &modules);
            let rendered = code_gen_context
                .tera
                .render("rust/mod.templ", &module_context)?;
            let mut file = File::create(output.join("ion_data_model/mod.rs"))?;
            file.write_all(rendered.as_bytes())?;
        }

        Ok(())
    }

    /// Generates data model based on given ISL type definition
    fn generate_data_model(
        output: &Path,
        modules: &mut Vec<String>,
        code_gen_context: &mut CodeGenContext,
        isl_type: &IslType,
    ) -> CodeGenResult<()> {
        let mut statements = vec![];

        let data_model_name = match isl_type.name().clone() {
            None => {
                format!("AnonymousType{}", code_gen_context.anonymous_type_counter)
            }
            Some(name) => name,
        };

        let mut context = Context::new();
        let mut tera_fields = vec![];
        let mut imports: Vec<Import> = vec![];

        // Set the name of the data model (i.e. enum/class)
        context.insert("name", &data_model_name.to_case(Case::UpperCamel));

        let constraints = isl_type.constraints();
        for constraint in constraints {
            Self::map_constraint_to_data_model(
                output,
                modules,
                code_gen_context,
                &mut tera_fields,
                &mut imports,
                constraint,
            )?;
        }

        // add imports for the template
        context.insert("imports", &imports);

        // generate read and write APIs for the data model
        Self::generate_read_api(
            code_gen_context,
            &mut statements,
            &mut context,
            &mut tera_fields,
        )?;
        Self::generate_write_api(code_gen_context, &mut context, &mut tera_fields);
        context.insert("statements", &statements);
        modules.push(code_gen_context.language.file_name(&data_model_name));

        // Render or generate file for the template with the given context
        let rendered = code_gen_context
            .tera
            .render(
                &format!(
                    "{}/{}.templ",
                    &code_gen_context.language,
                    code_gen_context.template_name()
                ),
                &context,
            )
            .unwrap();
        let mut file = File::create(output.join(format!(
            "ion_data_model/{}.{}",
            code_gen_context.language.file_name(&data_model_name),
            code_gen_context.language.file_extension()
        )))?;
        file.write_all(rendered.as_bytes())?;
        Ok(())
    }

    /// Maps the given constraint value to a data model
    fn map_constraint_to_data_model(
        output: &Path,
        modules: &mut Vec<String>,
        code_gen_context: &mut CodeGenContext,
        tera_fields: &mut Vec<Field>,
        imports: &mut Vec<Import>,
        constraint: &IslConstraint,
    ) -> CodeGenResult<()> {
        match constraint.constraint() {
            IslConstraintValue::Element(isl_type, _) => {
                Self::verify_data_model_consistency(code_gen_context, DataModel::SequenceStruct)?;
                Self::generate_struct_field(
                    code_gen_context,
                    tera_fields,
                    isl_type,
                    output,
                    modules,
                    "value",
                    imports,
                )?;
            }
            IslConstraintValue::Fields(fields, _content_closed) => {
                Self::verify_data_model_consistency(code_gen_context, DataModel::Struct)?;
                for (name, value) in fields.iter() {
                    Self::generate_struct_field(
                        code_gen_context,
                        tera_fields,
                        value.type_reference(),
                        output,
                        modules,
                        name,
                        imports,
                    )?;
                }
            }
            IslConstraintValue::Type(isl_type) => {
                Self::verify_data_model_consistency(code_gen_context, DataModel::UnitStruct)?;
                Self::generate_struct_field(
                    code_gen_context,
                    tera_fields,
                    isl_type,
                    output,
                    modules,
                    "value",
                    imports,
                )?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Verify that the current data model is same as previously determined data model
    fn verify_data_model_consistency(
        code_gen_context: &mut CodeGenContext,
        current_data_model: DataModel,
    ) -> CodeGenResult<()> {
        if let Some(data_model) = &code_gen_context.data_model {
            if data_model != &current_data_model {
                return invalid_data_model_error("Can not determine data model, constraints are mapping to different data models.");
            }
        } else {
            code_gen_context.with_data_model(current_data_model);
        }
        Ok(())
    }

    /// Generates a struct field based on field name and value(data type)
    fn generate_struct_field(
        code_gen_context: &mut CodeGenContext,
        tera_fields: &mut Vec<Field>,
        isl_type_ref: &IslTypeRef,
        output: &Path,
        modules: &mut Vec<String>,
        field_name: &str,
        imports: &mut Vec<Import>,
    ) -> CodeGenResult<()> {
        let value =
            Self::generate_field_value(code_gen_context, isl_type_ref, output, modules, imports)?;

        tera_fields.push(Field {
            name: {
                match code_gen_context.language {
                    Language::Rust => field_name.to_case(Case::Snake),
                    Language::Java => field_name.to_case(Case::Camel),
                }
            },
            value,
        });
        Ok(())
    }

    /// Generates field value in a struct which represents a data type in codegen's programming language
    fn generate_field_value(
        code_gen_context: &mut CodeGenContext,
        isl_type_ref: &IslTypeRef,
        output: &Path,
        modules: &mut Vec<String>,
        imports: &mut Vec<Import>,
    ) -> CodeGenResult<String> {
        Ok(match isl_type_ref {
            IslTypeRef::Named(name, _) => {
                if !Self::is_built_in_type(name) {
                    imports.push(Import {
                        module_name: name.to_case(Case::Snake),
                        type_name: name.to_case(Case::UpperCamel),
                    });
                }
                let name = Self::map_to_base_type(name, &code_gen_context.language);
                Self::generate_sequence_field_value(name, code_gen_context)
            }
            IslTypeRef::TypeImport(_, _) => {
                unimplemented!("Imports in schema are not supported yet!");
            }
            IslTypeRef::Anonymous(type_def, _) => {
                code_gen_context.anonymous_type_counter += 1;
                // store the parent data model
                let parent_data_model = code_gen_context.data_model.to_owned();
                code_gen_context.with_initial_data_model();
                Self::generate_data_model(output, modules, code_gen_context, type_def)?;
                // set back the parent data model
                if let Some(data_model) = parent_data_model {
                    code_gen_context.with_data_model(data_model)
                }

                let name = format!("AnonymousType{}", code_gen_context.anonymous_type_counter);
                imports.push(Import {
                    module_name: name.to_case(Case::Snake),
                    type_name: name.to_case(Case::UpperCamel),
                });
                Self::generate_sequence_field_value(name, code_gen_context)
            }
        })
    }

    /// Generates field value in a struct which represents a sequence data type in codegen's programming language
    pub fn generate_sequence_field_value(
        name: String,
        code_gen_context: &mut CodeGenContext,
    ) -> String {
        if code_gen_context.data_model == Some(DataModel::SequenceStruct) {
            return match code_gen_context.language {
                Language::Rust => {
                    format!("Vec<{}>", name)
                }
                Language::Java => {
                    format!("ArrayList<{}>", name)
                }
            };
        }
        name
    }

    /// Generates read API for a data model
    /// This adds statements for reading Ion value based on given data model that will be used by data model templates
    // TODO: add support for Java
    fn generate_read_api(
        code_gen_context: &mut CodeGenContext,
        statements: &mut Vec<String>,
        context: &mut Context,
        tera_fields: &mut Vec<Field>,
    ) -> CodeGenResult<()> {
        if code_gen_context.data_model == Some(DataModel::Struct)
            || code_gen_context.data_model == Some(DataModel::UnitStruct)
            || code_gen_context.data_model == Some(DataModel::SequenceStruct)
        {
            context.insert("fields", &tera_fields);
            if let Some(data_model) = &code_gen_context.data_model {
                context.insert("data_model", data_model);
            } else {
                return invalid_data_model_error(
                    "Can not determine data model, constraints are mapping to different data models.",
                );
            }

            for tera_field in tera_fields {
                if !Self::is_built_in_type(&tera_field.value) {
                    if code_gen_context.data_model == Some(DataModel::SequenceStruct) {
                        statements.push(format!(
                            "\"{}\" => {{ data_model.{} =",
                            &tera_field.name, &tera_field.name,
                        ));
                        statements.push(
                            r#"{
                let mut values = vec![];
                reader.step_in()?;
                while reader.next()? != StreamItem::Nothing {"#
                                .to_string(),
                        );
                        let sequence_type = &tera_field.value.replace("Vec<", "").replace('>', "");
                        if !Self::is_built_in_type(sequence_type) {
                            statements.push(format!(
                                "values.push({}::read_from(reader)?)",
                                sequence_type
                            ));
                        } else {
                            statements.push(format!(
                                "values.push(reader.read_{}()?)",
                                //TODO: there is an issue with how
                                sequence_type.to_lowercase()
                            ));
                        }

                        statements.push(
                            r#"}
                values }}"#
                                .to_string(),
                        );
                    } else if code_gen_context.data_model == Some(DataModel::UnitStruct) {
                        context.insert(
                            "read_statement",
                            &format!("{}::read_from(reader)?", &tera_field.value,),
                        );
                    } else {
                        statements.push(format!(
                            "\"{}\" => {{ data_model.{} = {}::read_from(reader)?;}}",
                            &tera_field.name, &tera_field.name, &tera_field.value,
                        ));
                    }
                } else {
                    if code_gen_context.data_model == Some(DataModel::UnitStruct) {
                        context.insert(
                            "read_statement",
                            &format!("reader.read_{}()?", &tera_field.value.to_lowercase(),),
                        );
                    }
                    statements.push(format!(
                        "\"{}\" => {{ data_model.{} = reader.read_{}()?;}}",
                        &tera_field.name,
                        &tera_field.name,
                        &tera_field.value.to_lowercase()
                    ));
                }
            }
        }
        Ok(())
    }

    /// Generates write API for a data model
    /// This adds statements for writing data model as Ion value that will be used by data model templates
    // TODO: add support for Java
    fn generate_write_api(
        code_gen_context: &mut CodeGenContext,
        context: &mut Context,
        tera_fields: &mut Vec<Field>,
    ) {
        let mut write_statements = Vec::new();
        if code_gen_context.data_model == Some(DataModel::UnitStruct) {
            for tera_field in tera_fields {
                if !Self::is_built_in_type(&tera_field.value) {
                    write_statements.push(format!("self.{}.write_to(writer)?;", &tera_field.name,));
                } else {
                    write_statements.push(format!(
                        "writer.write_{}(self.value)?;",
                        &tera_field.value.to_lowercase(),
                    ));
                }
            }
        } else if code_gen_context.data_model == Some(DataModel::Struct) {
            write_statements.push("writer.step_in(IonType::Struct)?;".to_string());
            for tera_field in tera_fields {
                write_statements.push(format!("writer.set_field_name(\"{}\");", &tera_field.name));

                if !Self::is_built_in_type(&tera_field.value) {
                    write_statements.push(format!("self.{}.write_to(writer)?;", &tera_field.name,));
                } else {
                    write_statements.push(format!(
                        "writer.write_{}(self.{})?;",
                        &tera_field.value.to_lowercase(),
                        &tera_field.name
                    ));
                }
            }
            write_statements.push("writer.step_out()?;".to_string());
        } else if code_gen_context.data_model == Some(DataModel::SequenceStruct) {
            write_statements.push("writer.step_in(IonType::List)?;".to_string());
            for tera_field in tera_fields {
                let sequence_type = &tera_field.value.replace("Vec<", "").replace('>', "");
                write_statements.push("for value in self.value {".to_string());
                if !Self::is_built_in_type(sequence_type) {
                    write_statements.push("value.write_to(writer)?;".to_string());
                } else {
                    write_statements.push(format!(
                        "writer.write_{}(value)?;",
                        &sequence_type.to_lowercase(),
                    ));
                }
                write_statements.push("}".to_string());
            }
            write_statements.push("writer.step_out()?;".to_string());
        }
        context.insert("write_statements", &write_statements);
    }
}

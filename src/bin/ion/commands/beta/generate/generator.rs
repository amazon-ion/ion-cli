use crate::commands::beta::generate::context::{CodeGenContext, DataModel};
use crate::commands::beta::generate::result::{invalid_data_model_error, CodeGenResult};
use crate::commands::beta::generate::utils::{Field, Import, Language};
use crate::commands::beta::generate::utils::{IonSchemaType, Template};
use convert_case::{Case, Casing};
use ion_schema::isl::isl_constraint::{IslConstraint, IslConstraintValue};
use ion_schema::isl::isl_type::IslType;
use ion_schema::isl::isl_type_reference::IslTypeRef;
use ion_schema::isl::IslSchema;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use tera::{Context, Tera};

// TODO: generator cna store language and output path as it doesn't change during code generation process
pub(crate) struct CodeGenerator<'a> {
    // Represents the templating engine - tera
    // more information: https://docs.rs/tera/latest/tera/
    pub(crate) tera: Tera,
    language: Language,
    output: &'a Path,
    // Represents a counter for naming anonymous type definitions
    pub(crate) anonymous_type_counter: usize,
}

impl<'a> CodeGenerator<'a> {
    pub fn new(language: Language, output: &'a Path) -> Self {
        Self {
            language,
            output,
            anonymous_type_counter: 0,
            tera: Tera::new("src/bin/ion/commands/beta/generate/templates/**/*.templ").unwrap(),
        }
    }

    /// Returns true if its a built in type otherwise returns false
    pub fn is_built_in_type(&self, name: &str) -> bool {
        match self.language {
            Language::Rust => {
                matches!(name, "i64" | "String" | "bool" | "Vec<u8>" | "f64")
            }
            Language::Java => {
                matches!(name, "int" | "String" | "boolean" | "byte[]" | "float")
            }
        }
    }

    /// Represents a [tera] filter that converts given tera string value to [upper camel case].
    /// Returns error if the given value is not a string.
    ///
    /// For more information: <https://docs.rs/tera/1.19.0/tera/struct.Tera.html#method.register_filter>
    ///
    /// [tera]: <https://docs.rs/tera/latest/tera/>
    /// [upper camel case]: <https://docs.rs/convert_case/latest/convert_case/enum.Case.html#variant.UpperCamel>
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
    pub fn generate(&mut self, schema: IslSchema) -> CodeGenResult<()> {
        // this will be used for Rust to create mod.rs which lists all the generates modules
        let mut modules = vec![];
        let mut module_context = tera::Context::new();

        // Register a tera filter that can be used to convert a string to upper camel case
        self.tera.register_filter("upper_camel", Self::upper_camel);

        for isl_type in schema.types() {
            self.generate_data_model(&mut modules, isl_type)?;
        }

        if self.language == Language::Rust {
            module_context.insert("modules", &modules);
            let rendered = self.tera.render("rust/mod.templ", &module_context)?;
            let mut file = File::create(self.output.join("mod.rs"))?;
            file.write_all(rendered.as_bytes())?;
        }

        Ok(())
    }

    /// Generates data model based on given ISL type definition
    fn generate_data_model(
        &mut self,
        modules: &mut Vec<String>,
        isl_type: &IslType,
    ) -> CodeGenResult<()> {
        let data_model_name = match isl_type.name().clone() {
            None => {
                format!("AnonymousType{}", self.anonymous_type_counter)
            }
            Some(name) => name,
        };

        let mut context = Context::new();
        let mut tera_fields = vec![];
        let mut imports: Vec<Import> = vec![];
        let mut code_gen_context = CodeGenContext::new();

        // Set the target kind name of the data model (i.e. enum/class)
        context.insert("name", &data_model_name.to_case(Case::UpperCamel));

        let constraints = isl_type.constraints();
        for constraint in constraints {
            self.map_constraint_to_data_model(
                modules,
                &mut tera_fields,
                &mut imports,
                constraint,
                &mut code_gen_context,
            )?;
        }

        // add imports for the template
        context.insert("imports", &imports);

        // generate read and write APIs for the data model
        self.generate_read_api(&mut context, &mut tera_fields, &mut code_gen_context)?;
        self.generate_write_api(&mut context, &mut tera_fields, &mut code_gen_context);
        modules.push(self.language.file_name(&data_model_name));

        // Render or generate file for the template with the given context
        let template: &Template = &code_gen_context.data_model.as_ref().try_into()?;
        let rendered = self
            .tera
            .render(
                &format!("{}/{}.templ", &self.language, template.name(&self.language)),
                &context,
            )
            .unwrap();
        let mut file = File::create(self.output.join(format!(
            "{}.{}",
            self.language.file_name(&data_model_name),
            self.language.file_extension()
        )))?;
        file.write_all(rendered.as_bytes())?;
        Ok(())
    }

    /// Maps the given constraint value to a data model
    fn map_constraint_to_data_model(
        &mut self,
        modules: &mut Vec<String>,
        tera_fields: &mut Vec<Field>,
        imports: &mut Vec<Import>,
        constraint: &IslConstraint,
        code_gen_context: &mut CodeGenContext,
    ) -> CodeGenResult<()> {
        match constraint.constraint() {
            IslConstraintValue::Element(isl_type, _) => {
                self.verify_data_model_consistency(DataModel::Sequence, code_gen_context)?;
                self.generate_struct_field(
                    tera_fields,
                    isl_type,
                    modules,
                    "value",
                    imports,
                    code_gen_context,
                )?;
            }
            IslConstraintValue::Fields(fields, _content_closed) => {
                self.verify_data_model_consistency(DataModel::Struct, code_gen_context)?;
                for (name, value) in fields.iter() {
                    self.generate_struct_field(
                        tera_fields,
                        value.type_reference(),
                        modules,
                        name,
                        imports,
                        code_gen_context,
                    )?;
                }
            }
            IslConstraintValue::Type(isl_type) => {
                self.verify_data_model_consistency(DataModel::Value, code_gen_context)?;
                self.generate_struct_field(
                    tera_fields,
                    isl_type,
                    modules,
                    "value",
                    imports,
                    code_gen_context,
                )?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Verify that the current data model is same as previously determined data model
    fn verify_data_model_consistency(
        &mut self,
        current_data_model: DataModel,
        code_gen_context: &mut CodeGenContext,
    ) -> CodeGenResult<()> {
        if let Some(data_model) = &code_gen_context.data_model {
            if data_model != &current_data_model {
                return invalid_data_model_error(format!("Can not determine abstract data type as current constraint {} conflicts with prior constraints for {}.", current_data_model, data_model));
            }
        } else {
            code_gen_context.with_data_model(current_data_model);
        }
        Ok(())
    }

    /// Generates a struct field based on field name and value(data type)
    fn generate_struct_field(
        &mut self,
        tera_fields: &mut Vec<Field>,
        isl_type_ref: &IslTypeRef,
        modules: &mut Vec<String>,
        field_name: &str,
        imports: &mut Vec<Import>,
        code_gen_context: &mut CodeGenContext,
    ) -> CodeGenResult<()> {
        let value = self.generate_field_value(isl_type_ref, modules, imports, code_gen_context)?;

        tera_fields.push(Field {
            name: {
                match self.language {
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
        &mut self,
        isl_type_ref: &IslTypeRef,
        modules: &mut Vec<String>,
        imports: &mut Vec<Import>,
        code_gen_context: &mut CodeGenContext,
    ) -> CodeGenResult<String> {
        Ok(match isl_type_ref {
            IslTypeRef::Named(name, _) => {
                if !self.is_built_in_type(name) {
                    imports.push(Import {
                        module_name: name.to_case(Case::Snake),
                        type_name: name.to_case(Case::UpperCamel),
                    });
                }
                let schema_type: IonSchemaType = name.into();
                self.generate_sequence_field_value(
                    schema_type.target_type(&self.language).to_string(),
                    code_gen_context,
                )
            }
            IslTypeRef::TypeImport(_, _) => {
                unimplemented!("Imports in schema are not supported yet!");
            }
            IslTypeRef::Anonymous(type_def, _) => {
                self.anonymous_type_counter += 1;
                self.generate_data_model(modules, type_def)?;
                let name = format!("AnonymousType{}", self.anonymous_type_counter);
                imports.push(Import {
                    module_name: name.to_case(Case::Snake),
                    type_name: name.to_case(Case::UpperCamel),
                });
                self.generate_sequence_field_value(name, code_gen_context)
            }
        })
    }

    /// Generates field value in a struct which represents a sequence data type in codegen's programming language
    pub fn generate_sequence_field_value(
        &mut self,
        name: String,
        code_gen_context: &mut CodeGenContext,
    ) -> String {
        if code_gen_context.data_model == Some(DataModel::Sequence) {
            return match self.language {
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
        &mut self,
        context: &mut Context,
        tera_fields: &mut Vec<Field>,
        code_gen_context: &mut CodeGenContext,
    ) -> CodeGenResult<()> {
        let mut statements = vec![];

        if code_gen_context.data_model == Some(DataModel::Struct)
            || code_gen_context.data_model == Some(DataModel::Value)
            || code_gen_context.data_model == Some(DataModel::Sequence)
        {
            context.insert("fields", &tera_fields);
            if let Some(data_model) = &code_gen_context.data_model {
                context.insert("data_model", data_model);
            } else {
                return invalid_data_model_error(
                    "Can not determine data model, constraints are mapping not mapping to a data model.",
                );
            }

            for tera_field in tera_fields {
                if !self.is_built_in_type(&tera_field.value) {
                    if code_gen_context.data_model == Some(DataModel::Sequence) {
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
                        if !self.is_built_in_type(sequence_type) {
                            statements.push(format!(
                                "values.push({}::read_from(reader)?)",
                                sequence_type
                            ));
                        } else {
                            statements.push(format!(
                                "values.push(reader.read_{}()?)",
                                sequence_type.to_lowercase()
                            ));
                        }

                        statements.push(
                            r#"}
                values }}"#
                                .to_string(),
                        );
                    } else if code_gen_context.data_model == Some(DataModel::Value) {
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
                    if code_gen_context.data_model == Some(DataModel::Value) {
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
        context.insert("statements", &statements);
        Ok(())
    }

    /// Generates write API for a data model
    /// This adds statements for writing data model as Ion value that will be used by data model templates
    // TODO: add support for Java
    fn generate_write_api(
        &mut self,
        context: &mut Context,
        tera_fields: &mut Vec<Field>,
        code_gen_context: &mut CodeGenContext,
    ) {
        let mut write_statements = Vec::new();
        if code_gen_context.data_model == Some(DataModel::Value) {
            for tera_field in tera_fields {
                if !self.is_built_in_type(&tera_field.value) {
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

                if !self.is_built_in_type(&tera_field.value) {
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
        } else if code_gen_context.data_model == Some(DataModel::Sequence) {
            write_statements.push("writer.step_in(IonType::List)?;".to_string());
            for tera_field in tera_fields {
                let sequence_type = &tera_field.value.replace("Vec<", "").replace('>', "");
                write_statements.push("for value in self.value {".to_string());
                if !self.is_built_in_type(sequence_type) {
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

use crate::commands::beta::generate::context::{AbstractDataType, CodeGenContext};
use crate::commands::beta::generate::result::{invalid_abstract_data_type_error, CodeGenResult};
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

// TODO: generator can store language and output path as it doesn't change during code generation process
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
        // this will be used for Rust to create mod.rs which lists all the generated modules
        let mut modules = vec![];
        let mut module_context = tera::Context::new();

        // Register a tera filter that can be used to convert a string to upper camel case
        self.tera.register_filter("upper_camel", Self::upper_camel);

        for isl_type in schema.types() {
            self.generate_abstract_data_type(&mut modules, isl_type)?;
        }

        if self.language == Language::Rust {
            module_context.insert("modules", &modules);
            let rendered = self.tera.render("rust/mod.templ", &module_context)?;
            let mut file = File::create(self.output.join("mod.rs"))?;
            file.write_all(rendered.as_bytes())?;
        }

        Ok(())
    }

    /// Generates abstract data type based on given ISL type definition
    fn generate_abstract_data_type(
        &mut self,
        modules: &mut Vec<String>,
        isl_type: &IslType,
    ) -> CodeGenResult<()> {
        let abstract_data_type_name = match isl_type.name().clone() {
            None => {
                format!("AnonymousType{}", self.anonymous_type_counter)
            }
            Some(name) => name,
        };

        let mut context = Context::new();
        let mut tera_fields = vec![];
        let mut imports: Vec<Import> = vec![];
        let mut code_gen_context = CodeGenContext::new();

        // Set the target kind name of the abstract data type (i.e. enum/class)
        context.insert(
            "target_kind_name",
            &abstract_data_type_name.to_case(Case::UpperCamel),
        );

        let constraints = isl_type.constraints();
        for constraint in constraints {
            self.map_constraint_to_abstract_data_type(
                modules,
                &mut tera_fields,
                &mut imports,
                constraint,
                &mut code_gen_context,
            )?;
        }

        // add imports for the template
        context.insert("imports", &imports);

        // generate read and write APIs for the abstract data type
        self.generate_read_api(&mut context, &mut tera_fields, &mut code_gen_context)?;
        self.generate_write_api(&mut context, &mut tera_fields, &mut code_gen_context);
        modules.push(self.language.file_name(&abstract_data_type_name));

        // Render or generate file for the template with the given context
        let template: &Template = &code_gen_context.abstract_data_type.as_ref().try_into()?;
        let rendered = self
            .tera
            .render(
                &format!("{}/{}.templ", &self.language, template.name(&self.language)),
                &context,
            )
            .unwrap();
        let mut file = File::create(self.output.join(format!(
            "{}.{}",
            self.language.file_name(&abstract_data_type_name),
            self.language.file_extension()
        )))?;
        file.write_all(rendered.as_bytes())?;
        Ok(())
    }

    /// Maps the given constraint value to an abstract data type
    fn map_constraint_to_abstract_data_type(
        &mut self,
        modules: &mut Vec<String>,
        tera_fields: &mut Vec<Field>,
        imports: &mut Vec<Import>,
        constraint: &IslConstraint,
        code_gen_context: &mut CodeGenContext,
    ) -> CodeGenResult<()> {
        match constraint.constraint() {
            IslConstraintValue::Element(isl_type, _) => {
                let type_name = self.type_reference_name(isl_type, modules, imports)?;
                self.verify_abstract_data_type_consistency(
                    AbstractDataType::Sequence(type_name.to_owned()),
                    code_gen_context,
                )?;
                self.generate_struct_field(tera_fields, type_name, "value", code_gen_context)?;
            }
            IslConstraintValue::Fields(fields, _content_closed) => {
                self.verify_abstract_data_type_consistency(
                    AbstractDataType::Struct,
                    code_gen_context,
                )?;
                for (name, value) in fields.iter() {
                    let type_name =
                        self.type_reference_name(value.type_reference(), modules, imports)?;

                    self.generate_struct_field(tera_fields, type_name, name, code_gen_context)?;
                }
            }
            IslConstraintValue::Type(isl_type) => {
                let type_name = self.type_reference_name(isl_type, modules, imports)?;

                self.verify_abstract_data_type_consistency(
                    AbstractDataType::Value,
                    code_gen_context,
                )?;
                self.generate_struct_field(tera_fields, type_name, "value", code_gen_context)?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Verify that the current abstract data type is same as previously determined abstract data type
    /// This is referring to abstract data type determined with each constraint that is verifies
    /// that all the constraints map to a single abstract data type and not different abstract data types.
    /// e.g.
    /// ```
    /// type::{
    ///   name: foo,
    ///   type: string,
    ///   fields:{
    ///      source: String,
    ///      destination: String
    ///   }
    /// }
    /// ```
    /// For the above schema, both `fields` and `type` constraints map to different abstract data types
    /// respectively Struct(with given fields `source` and `destination`) and Value(with a single field that has String data type).
    fn verify_abstract_data_type_consistency(
        &mut self,
        current_abstract_data_type: AbstractDataType,
        code_gen_context: &mut CodeGenContext,
    ) -> CodeGenResult<()> {
        if let Some(abstract_data_type) = &code_gen_context.abstract_data_type {
            if abstract_data_type != &current_abstract_data_type {
                return invalid_abstract_data_type_error(format!("Can not determine abstract data type as current constraint {} conflicts with prior constraints for {}.", current_abstract_data_type, abstract_data_type));
            }
        } else {
            code_gen_context.with_abstract_data_type(current_abstract_data_type);
        }
        Ok(())
    }

    /// Provides name of the type reference that will be used for generated abstract data type
    fn type_reference_name(
        &mut self,
        isl_type_ref: &IslTypeRef,
        modules: &mut Vec<String>,
        imports: &mut Vec<Import>,
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
                schema_type.target_type(&self.language).to_string()
            }
            IslTypeRef::TypeImport(_, _) => {
                unimplemented!("Imports in schema are not supported yet!");
            }
            IslTypeRef::Anonymous(type_def, _) => {
                self.anonymous_type_counter += 1;
                self.generate_abstract_data_type(modules, type_def)?;
                let name = format!("AnonymousType{}", self.anonymous_type_counter);
                imports.push(Import {
                    module_name: name.to_case(Case::Snake),
                    type_name: name.to_case(Case::UpperCamel),
                });
                name
            }
        })
    }

    /// Generates a struct field based on field name and value(data type)
    fn generate_struct_field(
        &mut self,
        tera_fields: &mut Vec<Field>,
        abstraxt_data_type_name: String,
        field_name: &str,
        code_gen_context: &mut CodeGenContext,
    ) -> CodeGenResult<()> {
        let value = self.generate_field_value(abstraxt_data_type_name, code_gen_context)?;

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
        abstract_data_type_name: String,
        code_gen_context: &mut CodeGenContext,
    ) -> CodeGenResult<String> {
        Ok(self.generate_sequence_field_value(abstract_data_type_name, code_gen_context))
    }

    /// Generates an appropriately typed sequence in the target programming language to use as a field value
    pub fn generate_sequence_field_value(
        &mut self,
        name: String,
        code_gen_context: &mut CodeGenContext,
    ) -> String {
        if code_gen_context.abstract_data_type == Some(AbstractDataType::Sequence(name.to_owned()))
        {
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

    /// Generates Generates a read API for an abstract data type.
    /// This adds statements for reading each the Ion value(s) that collectively represent the given abstract data type.
    // TODO: add support for Java
    fn generate_read_api(
        &mut self,
        context: &mut Context,
        tera_fields: &mut Vec<Field>,
        code_gen_context: &mut CodeGenContext,
    ) -> CodeGenResult<()> {
        let mut read_statements = vec![];

        if code_gen_context.abstract_data_type == Some(AbstractDataType::Struct)
            || code_gen_context.abstract_data_type == Some(AbstractDataType::Value)
            || matches!(
                code_gen_context.abstract_data_type,
                Some(AbstractDataType::Sequence(_))
            )
        {
            context.insert("fields", &tera_fields);
            if let Some(abstract_data_type) = &code_gen_context.abstract_data_type {
                context.insert("abstract_data_type", abstract_data_type);
            } else {
                return invalid_abstract_data_type_error(
                    "Can not determine abstract data type, constraints are mapping not mapping to an abstract data type.",
                );
            }

            for tera_field in tera_fields {
                if !self.is_built_in_type(&tera_field.value) {
                    if let Some(AbstractDataType::Sequence(sequence_type)) =
                        &code_gen_context.abstract_data_type
                    {
                        read_statements.push(format!(
                            "\"{}\" => {{ abstract_data_type.{} =",
                            &tera_field.name, &tera_field.name,
                        ));
                        read_statements.push(
                            r#"{
                let mut values = vec![];
                reader.step_in()?;
                while reader.next()? != StreamItem::Nothing {"#
                                .to_string(),
                        );
                        if !self.is_built_in_type(sequence_type) {
                            read_statements.push(format!(
                                "values.push({}::read_from(reader)?)",
                                sequence_type
                            ));
                        } else {
                            read_statements.push(format!(
                                "values.push(reader.read_{}()?)",
                                sequence_type.to_lowercase()
                            ));
                        }

                        read_statements.push(
                            r#"}
                values }}"#
                                .to_string(),
                        );
                    } else if code_gen_context.abstract_data_type == Some(AbstractDataType::Value) {
                        context.insert(
                            "read_statement",
                            &format!("{}::read_from(reader)?", &tera_field.value,),
                        );
                    } else {
                        read_statements.push(format!(
                            "\"{}\" => {{ abstract_data_type.{} = {}::read_from(reader)?;}}",
                            &tera_field.name, &tera_field.name, &tera_field.value,
                        ));
                    }
                } else {
                    if code_gen_context.abstract_data_type == Some(AbstractDataType::Value) {
                        context.insert(
                            "read_statement",
                            &format!("reader.read_{}()?", &tera_field.value.to_lowercase(),),
                        );
                    }
                    read_statements.push(format!(
                        "\"{}\" => {{ abstract_data_type.{} = reader.read_{}()?;}}",
                        &tera_field.name,
                        &tera_field.name,
                        &tera_field.value.to_lowercase()
                    ));
                }
            }
        }
        context.insert("statements", &read_statements);
        Ok(())
    }

    /// Generates write API for an abstract data type
    /// This adds statements for writing abstract data type as Ion value that will be used by abstract data type templates
    // TODO: add support for Java
    fn generate_write_api(
        &mut self,
        context: &mut Context,
        tera_fields: &mut Vec<Field>,
        code_gen_context: &mut CodeGenContext,
    ) {
        let mut write_statements = Vec::new();
        if code_gen_context.abstract_data_type == Some(AbstractDataType::Value) {
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
        } else if code_gen_context.abstract_data_type == Some(AbstractDataType::Struct) {
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
        } else if let Some(AbstractDataType::Sequence(sequence_type)) =
            &code_gen_context.abstract_data_type
        {
            write_statements.push("writer.step_in(IonType::List)?;".to_string());
            for _tera_field in tera_fields {
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

use crate::commands::beta::generate::context::{AbstractDataType, CodeGenContext};
use crate::commands::beta::generate::result::{invalid_abstract_data_type_error, CodeGenResult};
use crate::commands::beta::generate::utils::{Field, Import, JavaLanguage, Language, RustLanguage};
use crate::commands::beta::generate::utils::{IonSchemaType, Template};
use convert_case::{Case, Casing};
use ion_schema::isl::isl_constraint::{IslConstraint, IslConstraintValue};
use ion_schema::isl::isl_type::IslType;
use ion_schema::isl::isl_type_reference::IslTypeRef;
use ion_schema::isl::IslSchema;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::marker::PhantomData;
use std::path::Path;
use tera::{Context, Tera};

pub(crate) struct CodeGenerator<'a, L: Language> {
    // Represents the templating engine - tera
    // more information: https://docs.rs/tera/latest/tera/
    pub(crate) tera: Tera,
    output: &'a Path,
    // Represents a counter for naming anonymous type definitions
    pub(crate) anonymous_type_counter: usize,
    // Current type definition is root type or not
    is_root_type: bool,
    phantom: PhantomData<L>,
}

impl<'a> CodeGenerator<'a, RustLanguage> {
    pub fn new(output: &'a Path) -> CodeGenerator<RustLanguage> {
        Self {
            output,
            anonymous_type_counter: 0,
            tera: Tera::new("src/bin/ion/commands/beta/generate/templates/rust/*.templ").unwrap(),
            phantom: PhantomData,
            is_root_type: true,
        }
    }

    /// Generates code in Rust for given Ion Schema
    pub fn generate(&mut self, schema: IslSchema) -> CodeGenResult<()> {
        // this will be used for Rust to create mod.rs which lists all the generated modules
        let mut modules = vec![];
        let mut module_context = tera::Context::new();

        // Register a tera filter that can be used to convert a string based on case
        self.tera.register_filter("upper_camel", Self::upper_camel);
        self.tera.register_filter("snake", Self::snake);
        self.tera.register_filter("camel", Self::camel);

        // Register a tera filter that can be used to see if a type is built in data type or not
        self.tera
            .register_filter("is_built_in_type", Self::is_built_in_type);

        for isl_type in schema.types() {
            self.generate_abstract_data_type(&mut modules, isl_type)?;
        }

        self.generate_modules(&mut modules, &mut module_context)?;

        Ok(())
    }

    pub fn generate_modules(
        &mut self,
        modules: &mut Vec<String>,
        module_context: &mut Context,
    ) -> CodeGenResult<()> {
        module_context.insert("modules", &modules);
        let rendered = self.tera.render("mod.templ", module_context)?;
        let mut file = File::create(self.output.join("mod.rs"))?;
        file.write_all(rendered.as_bytes())?;
        Ok(())
    }
}

impl<'a> CodeGenerator<'a, JavaLanguage> {
    pub fn new(output: &'a Path) -> CodeGenerator<JavaLanguage> {
        Self {
            output,
            anonymous_type_counter: 0,
            tera: Tera::new("src/bin/ion/commands/beta/generate/templates/java/*.templ").unwrap(),
            phantom: PhantomData,
            is_root_type: true,
        }
    }

    /// Generates code in Java for given Ion Schema
    pub fn generate(&mut self, schema: IslSchema) -> CodeGenResult<()> {
        // this will be used for Rust to create mod.rs which lists all the generated modules
        let mut modules = vec![];

        // Register a tera filter that can be used to convert a string based on case
        self.tera.register_filter("upper_camel", Self::upper_camel);
        self.tera.register_filter("snake", Self::snake);
        self.tera.register_filter("camel", Self::camel);

        for isl_type in schema.types() {
            self.generate_abstract_data_type(&mut modules, isl_type)?;
        }

        Ok(())
    }
}

impl<'a, L: Language> CodeGenerator<'a, L> {
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
                .ok_or(tera::Error::msg(
                    "the `upper_camel` filter only accepts strings",
                ))?
                .to_case(Case::UpperCamel),
        ))
    }

    /// Represents a [tera] filter that converts given tera string value to [camel case].
    /// Returns error if the given value is not a string.
    ///
    /// For more information: <https://docs.rs/tera/1.19.0/tera/struct.Tera.html#method.register_filter>
    ///
    /// [tera]: <https://docs.rs/tera/latest/tera/>
    /// [camel case]: <https://docs.rs/convert_case/latest/convert_case/enum.Case.html#variant.Camel>
    pub fn camel(
        value: &tera::Value,
        _map: &HashMap<String, tera::Value>,
    ) -> Result<tera::Value, tera::Error> {
        Ok(tera::Value::String(
            value
                .as_str()
                .ok_or(tera::Error::msg("Required string for this filter"))?
                .to_case(Case::Camel),
        ))
    }

    /// Represents a [tera] filter that converts given tera string value to [snake case].
    /// Returns error if the given value is not a string.
    ///
    /// For more information: <https://docs.rs/tera/1.19.0/tera/struct.Tera.html#method.register_filter>
    ///
    /// [tera]: <https://docs.rs/tera/latest/tera/>
    /// [snake case]: <https://docs.rs/convert_case/latest/convert_case/enum.Case.html#variant.Camel>
    pub fn snake(
        value: &tera::Value,
        _map: &HashMap<String, tera::Value>,
    ) -> Result<tera::Value, tera::Error> {
        Ok(tera::Value::String(
            value
                .as_str()
                .ok_or(tera::Error::msg("Required string for this filter"))?
                .to_case(Case::Snake),
        ))
    }

    /// Represents a [tera] filter that return true if the value is a built in type, otherwise returns false.
    ///
    /// For more information: <https://docs.rs/tera/1.19.0/tera/struct.Tera.html#method.register_filter>
    ///
    /// [tera]: <https://docs.rs/tera/latest/tera/>
    pub fn is_built_in_type(
        value: &tera::Value,
        _map: &HashMap<String, tera::Value>,
    ) -> Result<tera::Value, tera::Error> {
        Ok(tera::Value::Bool(L::is_built_in_type(
            value.as_str().ok_or(tera::Error::msg(
                "`is_built_in_type` called with non-String Value",
            ))?,
        )))
    }

    fn generate_abstract_data_type(
        &mut self,
        modules: &mut Vec<String>,
        isl_type: &IslType,
    ) -> CodeGenResult<()> {
        let isl_type_name = isl_type
            .name()
            .clone()
            .unwrap_or_else(|| format!("AnonymousType{}", self.anonymous_type_counter));

        let mut context = Context::new();
        let mut tera_fields = vec![];
        let mut imports: Vec<Import> = vec![];
        let mut code_gen_context = CodeGenContext::new();

        if self.is_root_type {
            context.insert("is_root_type", &true);
            self.is_root_type = false;
        } else {
            context.insert("is_root_type", &false);
        }

        // Set the ISL type name for the generated abstract data type
        context.insert("target_kind_name", &isl_type_name.to_case(Case::UpperCamel));

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

        // add fields for template
        // TODO: verify the `occurs` value within a field, by default the fields are optional.
        if let Some(abstract_data_type) = &code_gen_context.abstract_data_type {
            context.insert("fields", &tera_fields);
            context.insert("abstract_data_type", abstract_data_type);
        } else {
            return invalid_abstract_data_type_error(
                    "Can not determine abstract data type, constraints are mapping not mapping to an abstract data type.",
                );
        }

        self.render_generated_code(modules, &isl_type_name, &mut context, &mut code_gen_context)
    }

    fn render_generated_code(
        &mut self,
        modules: &mut Vec<String>,
        abstract_data_type_name: &str,
        context: &mut Context,
        code_gen_context: &mut CodeGenContext,
    ) -> CodeGenResult<()> {
        modules.push(L::file_name_for_type(abstract_data_type_name));

        // Render or generate file for the template with the given context
        let template: &Template = &code_gen_context.abstract_data_type.as_ref().try_into()?;
        let rendered = self
            .tera
            .render(&format!("{}.templ", L::template_name(template)), context)
            .unwrap();
        let mut file = File::create(self.output.join(format!(
            "{}.{}",
            L::file_name_for_type(abstract_data_type_name),
            L::file_extension()
        )))?;
        file.write_all(rendered.as_bytes())?;
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
                if !L::is_built_in_type(name) {
                    imports.push(Import {
                        name: name.to_string(),
                    });
                }
                let schema_type: IonSchemaType = name.into();
                L::target_type(&schema_type)
            }
            IslTypeRef::TypeImport(_, _) => {
                unimplemented!("Imports in schema are not supported yet!");
            }
            IslTypeRef::Anonymous(type_def, _) => {
                self.anonymous_type_counter += 1;
                let name = format!("AnonymousType{}", self.anonymous_type_counter);
                self.generate_abstract_data_type(modules, type_def)?;
                imports.push(Import {
                    name: name.to_string(),
                });
                name
            }
        })
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
                self.generate_struct_field(
                    tera_fields,
                    L::target_type_as_sequence(&type_name),
                    "value",
                )?;
            }
            IslConstraintValue::Fields(fields, content_closed) => {
                // TODO: Check for `closed` annotation on fields and based on that return error while reading if there are extra fields.
                self.verify_abstract_data_type_consistency(
                    AbstractDataType::Structure(*content_closed),
                    code_gen_context,
                )?;
                for (name, value) in fields.iter() {
                    let type_name =
                        self.type_reference_name(value.type_reference(), modules, imports)?;

                    self.generate_struct_field(tera_fields, type_name, name)?;
                }
            }
            IslConstraintValue::Type(isl_type) => {
                let type_name = self.type_reference_name(isl_type, modules, imports)?;

                self.verify_abstract_data_type_consistency(
                    AbstractDataType::Value,
                    code_gen_context,
                )?;
                self.generate_struct_field(tera_fields, type_name, "value")?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Generates a struct field based on field name and value(data type)
    fn generate_struct_field(
        &mut self,
        tera_fields: &mut Vec<Field>,
        abstract_data_type_name: String,
        field_name: &str,
    ) -> CodeGenResult<()> {
        tera_fields.push(Field {
            name: field_name.to_string(),
            value: abstract_data_type_name,
        });
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
}

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;
    use ion_rs::Element;
    use ion_rs::IonType;
    use ion_rs::ReaderBuilder;
    use ion_rs::TextWriterBuilder;
    use std::fs;
    use std::path::MAIN_SEPARATOR_STR as PATH_SEPARATOR;
    use test_generator::test_resources;

    include!(concat!(env!("OUT_DIR"), "/ion_generated_code.rs"));

    /// Determines if the given file name is in the ROUNDTRIP_TESTS_SKIP_LIST list. This deals with platform
    /// path separator differences from '/' separators in the path list.
    #[inline]
    pub fn skip_list_contains_path(file_name: &str) -> bool {
        ROUNDTRIP_TESTS_SKIP_LIST
            .iter()
            // TODO construct the paths in a not so hacky way
            .map(|p| p.replace('/', PATH_SEPARATOR))
            .any(|p| p == file_name)
    }

    pub const ROUNDTRIP_TESTS_SKIP_LIST: &[&str] = &[
        "../../input/good/nested_struct/valid_optional_fields.ion",
        "../../input/good/struct_with_fields/valid_optional_fields.ion",
        "../../input/bad/struct_with_fields/missing_required_fields.ion",
    ];

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }

    #[test_resources("../../input/good/struct_with_fields/**/*.ion")]
    fn roundtrip_good_test_generated_code_structs_with_fields(file_name: &str) -> SerdeResult<()> {
        // if file name is under the ROUNDTRIP_TESTS_SKIP_LIST then do nothing.
        if skip_list_contains_path(&file_name) {
            return Ok(());
        }
        let ion_string = fs::read_to_string(file_name).unwrap();
        let mut reader = ReaderBuilder::new().build(ion_string.clone())?;
        let mut buffer = Vec::new();
        let mut text_writer = TextWriterBuilder::default().build(&mut buffer)?;
        // read given Ion value using Ion reader
        reader.next()?;
        let structs_with_fields: StructWithFields = StructWithFields::read_from(&mut reader)?;
        // write the generated abstract data type using Ion writer
        structs_with_fields.write_to(&mut text_writer)?;
        text_writer.flush()?;
        // compare given Ion value with round tripped Ion value written using abstract data type's `write_to` API
        assert_eq!(
            Element::read_one(text_writer.output().as_slice())?,
            (Element::read_one(&ion_string)?)
        );

        Ok(())
    }

    #[test_resources("../../input/bad/struct_with_fields/**/*.ion")]
    fn roundtrip_bad_test_generated_code_structs_with_fields(file_name: &str) -> SerdeResult<()> {
        if skip_list_contains_path(&file_name) {
            return Ok(());
        }
        let ion_string = fs::read_to_string(file_name).unwrap();
        let mut reader = ReaderBuilder::new().build(ion_string.clone())?;
        // read given Ion value using Ion reader
        reader.next()?;
        let result = StructWithFields::read_from(&mut reader);
        assert!(result.is_err());

        Ok(())
    }

    #[test_resources("../../input/good/nested_struct/**/*.ion")]
    fn roundtrip_good_test_generated_code_nested_structs(file_name: &str) -> SerdeResult<()> {
        if skip_list_contains_path(&file_name) {
            return Ok(());
        }
        let ion_string = fs::read_to_string(file_name).unwrap();
        let mut reader = ReaderBuilder::new().build(ion_string.clone())?;
        let mut buffer = Vec::new();
        let mut text_writer = TextWriterBuilder::default().build(&mut buffer)?;
        // read given Ion value using Ion reader
        reader.next()?;
        let nested_struct: NestedStruct = NestedStruct::read_from(&mut reader)?;
        // write the generated abstract data type using Ion writer
        nested_struct.write_to(&mut text_writer)?;
        text_writer.flush()?;
        // compare given Ion value with round tripped Ion value written using abstract data type's `write_to` API
        assert_eq!(
            Element::read_one(text_writer.output().as_slice())?,
            (Element::read_one(&ion_string)?)
        );

        Ok(())
    }

    #[test_resources("../../input/bad/nested_struct/**/*.ion")]
    fn roundtrip_bad_test_generated_code_nested_structs(file_name: &str) -> SerdeResult<()> {
        let ion_string = fs::read_to_string(file_name).unwrap();
        let mut reader = ReaderBuilder::new().build(ion_string.clone())?;
        // read given Ion value using Ion reader
        reader.next()?;
        let result = NestedStruct::read_from(&mut reader);
        assert!(result.is_err());

        Ok(())
    }

    #[test_resources("../../input/good/scalar/**/*.ion")]
    fn roundtrip_good_test_generated_code_scalar(file_name: &str) -> SerdeResult<()> {
        let ion_string = fs::read_to_string(file_name).unwrap();
        let mut reader = ReaderBuilder::new().build(ion_string.clone())?;
        let mut buffer = Vec::new();
        let mut text_writer = TextWriterBuilder::default().build(&mut buffer)?;
        // read given Ion value using Ion reader
        reader.next()?;
        let scalar: Scalar = Scalar::read_from(&mut reader)?;
        // write the generated abstract data type using Ion writer
        scalar.write_to(&mut text_writer)?;
        text_writer.flush()?;
        // compare given Ion value with round tripped Ion value written using abstract data type's `write_to` API
        assert_eq!(
            Element::read_one(text_writer.output().as_slice())?,
            (Element::read_one(&ion_string)?)
        );

        Ok(())
    }

    #[test_resources("../../input/bad/scalar/**/*.ion")]
    fn roundtrip_bad_test_generated_code_scalar(file_name: &str) -> SerdeResult<()> {
        let ion_string = fs::read_to_string(file_name).unwrap();
        let mut reader = ReaderBuilder::new().build(ion_string.clone())?;
        // read given Ion value using Ion reader
        reader.next()?;
        let result = Scalar::read_from(&mut reader);
        assert!(result.is_err());

        Ok(())
    }

    #[test_resources("../../input/good/sequence/**/*.ion")]
    fn roundtrip_good_test_generated_code_sequence(file_name: &str) -> SerdeResult<()> {
        let ion_string = fs::read_to_string(file_name).unwrap();
        let mut reader = ReaderBuilder::new().build(ion_string.clone())?;
        let mut buffer = Vec::new();
        let mut text_writer = TextWriterBuilder::default().build(&mut buffer)?;
        // read given Ion value using Ion reader
        reader.next()?;
        let sequence: Sequence = Sequence::read_from(&mut reader)?;
        // write the generated abstract data type using Ion writer
        sequence.write_to(&mut text_writer)?;
        text_writer.flush()?;
        // compare given Ion value with round tripped Ion value written using abstract data type's `write_to` API
        assert_eq!(
            Element::read_one(text_writer.output().as_slice())?,
            (Element::read_one(&ion_string)?)
        );

        Ok(())
    }

    #[test_resources("../../input/bad/sequence/**/*.ion")]
    fn roundtrip_bad_test_generated_code_sequence(file_name: &str) -> SerdeResult<()> {
        let ion_string = fs::read_to_string(file_name).unwrap();
        let mut reader = ReaderBuilder::new().build(ion_string.clone())?;
        // read given Ion value using Ion reader
        reader.next()?;
        let result = Sequence::read_from(&mut reader);
        assert!(result.is_err());

        Ok(())
    }
}

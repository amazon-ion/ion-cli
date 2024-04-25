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
    use test_generator::test_resources;

    include!(concat!(env!("OUT_DIR"), "/ion_generated_code.rs"));

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }

    #[test_resources("../../input/good/struct_with_fields/**/*.ion")]
    fn roundtrip_good_test_generated_code_structs_with_fields(file_name: &str) -> IonResult<()> {
        let ion_string = fs::read_to_string(file_name)?;
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
    fn roundtrip_bad_test_generated_code_structs_with_fields(file_name: &str) -> IonResult<()> {
        let ion_string = fs::read_to_string(file_name)?;
        let mut reader = ReaderBuilder::new().build(ion_string.clone())?;
        // read given Ion value using Ion reader
        reader.next()?;
        let result = StructWithFields::read_from(&mut reader);
        assert!(result.is_err());

        Ok(())
    }

    #[test_resources("../../input/good/nested_struct/**/*.ion")]
    fn roundtrip_good_test_generated_code_nested_structs(file_name: &str) -> IonResult<()> {
        let ion_string = fs::read_to_string(file_name)?;
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
    fn roundtrip_bad_test_generated_code_nested_structs(file_name: &str) -> IonResult<()> {
        let ion_string = fs::read_to_string(file_name)?;
        let mut reader = ReaderBuilder::new().build(ion_string.clone())?;
        // read given Ion value using Ion reader
        reader.next()?;
        let result = NestedStruct::read_from(&mut reader);
        assert!(result.is_err());

        Ok(())
    }
}

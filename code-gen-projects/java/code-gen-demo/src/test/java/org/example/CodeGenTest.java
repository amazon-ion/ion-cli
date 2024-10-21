package org.example;

import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;
import java.util.ArrayList;
import com.amazon.ion.system.IonReaderBuilder;
import com.amazon.ion.IonReader;
import com.amazon.ion.system.IonTextWriterBuilder;
import com.amazon.ion.IonWriter;
import com.amazon.ion.IonSystem;
import com.amazon.ion.system.IonSystemBuilder;
import com.amazon.ion.IonLoader;
import com.amazon.ion.IonException;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.FileInputStream;
import java.io.BufferedInputStream;
import java.io.File;

class CodeGenTest {
    private static final IonSystem ionSystem = IonSystemBuilder.standard().build();
    private static final IonLoader ionLoader = ionSystem.getLoader();

    @Test void builderTestForStructWithFields() {
         StructWithFields.Builder sb = new StructWithFields.Builder();
         ArrayList<String> c = new ArrayList<String>();
         c.add("foo");
         c.add("bar");
         c.add("baz");

         // set all the fields of `StructWithFields`
         StructWithFields s = sb.a("hello").b(12).c(c).d(10e2).build();

         // getter tests for `StructWithFields`
         assertEquals("hello", s.getA(), "s.getA() should return \"hello\"");
         assertEquals(12, s.getB(), "s.getB() should return `12`");
         assertEquals(3, s.getC().size(), "s.getC() should return ArrayList fo size 3");
         assertEquals(10e2, s.getD(), "s.getD() should return `10e2`");
    }

    @Test void builderTestForNestedStruct() {
         // getter tests for `NestedStruct`
         NestedStruct.Builder nb = new NestedStruct.Builder();
         ArrayList<Integer> e = new ArrayList<Integer>();
         e.add(1);
         e.add(2);
         e.add(3);

         // set all the fields of `NestedStruct`
         NestedStruct.NestedType1.Builder nb1 = new NestedStruct.NestedType1.Builder();
         NestedStruct.NestedType1 c = nb1.d(false).e(e).build();
         NestedStruct n = nb.a("hello").b(12).c(c).build();

         // getter tests for `NestedStruct`
         assertEquals("hello", n.getA(), "n.getA() should return \"hello\"");
         assertEquals(12, n.getB(), "n.getB() should return `12`");
         assertEquals(false, n.getC().getD(), "n.getC().getD() should return `false`");
         assertEquals(3, n.getC().getE().size(), "n.getC().getE().size() should return ArrayList fo size 3");

          // setter tests for `NestedStruct`
          n.setA("hi");
          assertEquals("hi", n.getA(), "s.getA() should return \"hi\"");
          n.setB(6);
          assertEquals(6, n.getB(), "s.getB() should return `6`");
          n.getC().setD(true);
          assertEquals(true, n.getC().getD(), "s.getC().getD() should return `true`");
          n.getC().setE(new ArrayList<Integer>());
          assertEquals(0, n.getC().getE().size(), "s.getC().getE().size() should return ArrayList fo size 0");
    }

    @Test void getterAndSetterTestForSequence() {
         ArrayList<String> a = new ArrayList<String>();
         a.add("foo");
         a.add("bar");
         a.add("baz");
         Sequence s = new Sequence();

         // set all the fields of `Sequence`
         s.setValue(a);

         // getter tests for `Sequence`
         assertEquals(3, s.getValue().size(), "s.getValue().size() should return ArrayList fo size 3");
    }

    @Test void getterAndSetterTestForScalar() {
         Scalar s = new Scalar();

         // set all the fields of `Scalar`
         s.setValue("hello");

         // getter tests for `Scalar`
         assertEquals("hello", s.getValue(), "s.getValue() should return \"hello\"");

         // setter tests for `Scalar`
         s.setValue("hi");
         assertEquals("hi", s.getValue(), "s.getValue() should return \"hi\"");
    }

    @FunctionalInterface
    interface ReaderFunction<T> {
        T read(IonReader reader) throws IOException;
    }

    @FunctionalInterface
    interface WriterFunction<T> {
        void write(T item, IonWriter writer) throws IOException;
    }

    @Test
    void roundtripBadTestForScalar() throws IOException {
        runRoundtripBadTest("/bad/scalar", Scalar::readFrom);
    }

    @Test
    void roundtripBadTestForSequence() throws IOException {
        runRoundtripBadTest("/bad/sequence", Sequence::readFrom);
    }

    @Test
    void roundtripBadTestForStructWithFields() throws IOException {
        runRoundtripBadTest("/bad/struct_with_fields", StructWithFields::readFrom);
    }

    @Test
    void roundtripBadTestForNestedStruct() throws IOException {
        runRoundtripBadTest("/bad/nested_struct", NestedStruct::readFrom);
    }

    @Test
    void roundtripBadTestForStructWithEnumFields() throws IOException {
        runRoundtripBadTest("/bad/struct_with_enum_fields", StructWithEnumFields::readFrom);
    }

    @Test
    void roundtripBadTestForEnum() throws IOException {
        runRoundtripBadTest("/bad/enum", Enum::readFrom);
    }

    private <T> void runRoundtripBadTest(String path, ReaderFunction<T> readerFunction) throws IOException {
        File dir = new File(System.getenv("ION_INPUT") + path);
        String[] fileNames = dir.list();
        for (String fileName : fileNames) {
            File f = new File(dir, fileName);
            try (InputStream inputStream = new FileInputStream(f);
                    BufferedInputStream bufferedStream = new BufferedInputStream(inputStream);
                    IonReader reader = IonReaderBuilder.standard().build(bufferedStream)) {
                reader.next();
                assertThrows(Throwable.class, () -> readerFunction.read(reader));
            }
        }
    }

    @Test
    void roundtripGoodTestForScalar() throws IOException {
        runRoundtripGoodTest("/good/scalar", Scalar::readFrom, (item, writer) -> item.writeTo(writer));
    }

    @Test
    void roundtripGoodTestForSequence() throws IOException {
        runRoundtripGoodTest("/good/sequence", Sequence::readFrom, (item, writer) -> item.writeTo(writer));
    }

    @Test
    void roundtripGoodTestForStructWithFields() throws IOException {
        runRoundtripGoodTest("/good/struct_with_fields", StructWithFields::readFrom, (item, writer) -> item.writeTo(writer));
    }

    @Test
    void roundtripGoodTestForNestedStruct() throws IOException {
        runRoundtripGoodTest("/good/nested_struct", NestedStruct::readFrom, (item, writer) -> item.writeTo(writer));
    }

    @Test
    void roundtripGoodTestForStructWithEnumFields() throws IOException {
        runRoundtripGoodTest("/good/struct_with_enum_fields", StructWithEnumFields::readFrom, (item, writer) -> item.writeTo(writer));
    }


    @Test
    void roundtripGoodTestForEnum() throws IOException {
        runRoundtripGoodTest("/good/enum", Enum::readFrom, (item, writer) -> item.writeTo(writer));
    }

    private <T> void runRoundtripGoodTest(String path, ReaderFunction<T> readerFunction, WriterFunction<T> writerFunction) throws IOException {
        File dir = new File(System.getenv("ION_INPUT") + path);
        String[] fileNames = dir.list();
        for (String fileName : fileNames) {
            File f = new File(dir, fileName);
            InputStream inputStream = new FileInputStream(f);
            BufferedInputStream bufferedStream = new BufferedInputStream(inputStream);
            IonTextWriterBuilder b = IonTextWriterBuilder.standard();
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            IonReaderBuilder readerBuilder = IonReaderBuilder.standard();
            try (IonReader reader = readerBuilder.build(bufferedStream)) {
                reader.next();
                IonWriter writer = b.build(out);
                T item = readerFunction.read(reader);
                writerFunction.write(item, writer);
                writer.close();
                assertEquals(ionLoader.load(f), ionLoader.load(out.toByteArray()));
            }
        }
    }
}

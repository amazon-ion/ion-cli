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

    @Test void getterAndSetterTestForStructWithFields() {
         StructWithFields s = new StructWithFields();

         // set all the fields of `StructWithFields`
         s.setA(java.util.Optional.of("hello"));
         s.setB(java.util.Optional.of(12));
         s.setD(java.util.Optional.of(10e2));

         // getter tests for `StructWithFields`
         assertEquals("hello", s.getA().get(), "s.getA() should return \"hello\"");
         assertEquals(12, s.getB().get(), "s.getB() should return `12`");
         assertEquals(10e2, s.getD().get(), "s.getD() should return `10e2`");

         // setter tests for `StructWithFields`
         s.setA(java.util.Optional.of("hi"));
         assertEquals("hi", s.getA().get(), "s.getA() should return \"hi\"");
         s.setB(java.util.Optional.of(6));
         assertEquals(6, s.getB().get(), "s.getB() should return `6`");
         s.setD(java.util.Optional.of(11e3));
         assertEquals(11e3 ,s.getD().get(), "s.getD() should return `11e3`");
    }

    @Test void getterAndSetterTestForNestedStruct() {
         // getter tests for `NestedStruct`
         NestedStruct n = new NestedStruct();
         ArrayList<Integer> a = new ArrayList<Integer>();
         a.add(1);
         a.add(2);
         a.add(3);

         // set all the fields of `NestedStruct`
         n.setA(java.util.Optional.of("hello"));
         n.setB(java.util.Optional.of(12));
         n.setC(java.util.Optional.of(false), java.util.Optional.of(a));

         // getter tests for `NestedStruct`
         assertEquals("hello", n.getA().get(), "n.getA() should return \"hello\"");
         assertEquals(12, n.getB().get(), "n.getB() should return `12`");
         assertEquals(false, n.getC().get().getD().get(), "n.getC().getD() should return `false`");
         assertEquals(3, n.getC().get().getE().get().size(), "n.getC().getE().size() should return ArrayList fo size 3");

          // setter tests for `NestedStruct`
          n.setA(java.util.Optional.of("hi"));
          assertEquals("hi", n.getA().get(), "s.getA() should return \"hi\"");
          n.setB(java.util.Optional.of(6));
          assertEquals(6, n.getB().get(), "s.getB() should return `6`");
          n.getC().get().setD(java.util.Optional.of(true));
          assertEquals(true, n.getC().get().getD().get(), "s.getC().getD() should return `true`");
          n.getC().get().setE(java.util.Optional.of(new ArrayList<Integer>()));
          assertEquals(0, n.getC().get().getE().get().size(), "s.getC().getE().size() should return ArrayList fo size 0");
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

         // setter tests for `Sequence`
         s.setValue(new ArrayList<String>());
         assertEquals(true, s.getValue().isEmpty(), "s.getValue().isEmpty() should return `true`");
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

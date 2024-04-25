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
import java.io.File;

class CodeGenTest {
    private static final IonSystem ionSystem = IonSystemBuilder.standard().build();
    private static final IonLoader ionLoader = ionSystem.getLoader();

    @Test void getterAndSetterTestForStructWithFields() {
         ArrayList<String> a = new ArrayList<String>();
         a.add("foo");
         a.add("bar");
         a.add("baz");
         StructWithFields s = new StructWithFields("hello", 12, new AnonymousType2(a), 10e2);

         // getter tests for `StructWithFields`
         assertEquals("hello", s.getA(), "s.getA() should return \"hello\"");
         assertEquals(12, s.getB(), "s.getB() should return `12`");
         assertEquals(3, s.getC().getValue().size(), "s.getC().getValue() should return ArrayList fo size 3");
         assertEquals(10e2, s.getD(), "s.getD() should return `10e2`");

         // setter tests for `StructWithFields`
         s.setA("hi");
         assertEquals("hi", s.getA(), "s.getA() should return \"hi\"");
         s.setB(6);
         assertEquals(6, s.getB(), "s.getB() should return `6`");
         s.setC(new AnonymousType2(new ArrayList<String>()));
         assertEquals(true, s.getC().getValue().isEmpty(), "s.getC().isEmpty() should return `true`");
         s.setD(11e3);
         assertEquals(11e3 ,s.getD(), "s.getD() should return `11e3`");
    }

    @Test void getterAndSetterTestForNestedStruct() {
         // getter tests for `NestedStruct`
         NestedStruct n = new NestedStruct("hello", 12, new AnonymousType1(false));
         assertEquals("hello", n.getA(), "n.getA() should return \"hello\"");
         assertEquals(12, n.getB(), "n.getB() should return `12`");
         assertEquals(false, n.getC().getD(), "n.getC().getD() should return `false`");

         // setter tests for `NestedStruct`
         n.setA("hi");
         assertEquals("hi", n.getA(), "s.getA() should return \"hi\"");
         n.setB(6);
         assertEquals(6, n.getB(), "s.getB() should return `6`");
         n.getC().setD(true);
         assertEquals(true, n.getC().getD(), "s.getC().getD() should return `true`");
    }

    @Test void roundtripGoodTestForStructWithFields() throws IOException {
        File dir = new File("./../../input/good/struct_with_fields");
        String[] fileNames = dir.list();
        for (String fileName : fileNames) {
            File f = new File(dir, fileName);
            InputStream inputStream = new FileInputStream(f);
            IonTextWriterBuilder b = IonTextWriterBuilder.standard();
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            IonReaderBuilder readerBuilder = IonReaderBuilder.standard();
            try (IonReader reader = readerBuilder.build(inputStream)) {
                reader.next();
                StructWithFields s = StructWithFields.readFrom(reader);
                IonWriter writer = b.build(out);
                s.writeTo(writer);
                writer.close();
                assertEquals(ionLoader.load(f), ionLoader.load(out.toByteArray()));
            }
        }
    }

    @Test void roundtripBadTestForStructWithFields() throws IOException {
        File dir = new File("./../../input/bad/struct_with_fields");
        String[] fileNames = dir.list();
        for (String fileName : fileNames) {
            File f = new File(dir, fileName);
            InputStream inputStream = new FileInputStream(f);
            IonTextWriterBuilder b = IonTextWriterBuilder.standard();
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            IonReaderBuilder readerBuilder = IonReaderBuilder.standard();
            try (IonReader reader = readerBuilder.build(inputStream)) {
                reader.next();
                assertThrows(Throwable.class, () -> { StructWithFields s = StructWithFields.readFrom(reader); });
            }
        }
    }

    @Test void roundtripGoodTestForNestedStruct() throws IOException {
        File dir = new File("./../../input/good/nested_struct");
        String[] fileNames = dir.list();
        for (String fileName : fileNames) {
            File f = new File(dir, fileName);
            InputStream inputStream = new FileInputStream(f);
            IonTextWriterBuilder b = IonTextWriterBuilder.standard();
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            IonReaderBuilder readerBuilder = IonReaderBuilder.standard();
            try (IonReader reader = readerBuilder.build(inputStream)) {
                reader.next();
                NestedStruct n = NestedStruct.readFrom(reader);
                IonWriter writer = b.build(out);
                n.writeTo(writer);
                writer.close();
                assertEquals(ionLoader.load(f), ionLoader.load(out.toByteArray()));
            }
        }
    }

    @Test void roundtripBadTestForNestedStruct() throws IOException {
        File dir = new File("./../../input/bad/nested_struct");
        String[] fileNames = dir.list();
        for (String fileName : fileNames) {
            File f = new File(dir, fileName);
            InputStream inputStream = new FileInputStream(f);
            IonTextWriterBuilder b = IonTextWriterBuilder.standard();
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            IonReaderBuilder readerBuilder = IonReaderBuilder.standard();
            try (IonReader reader = readerBuilder.build(inputStream)) {
                reader.next();
                assertThrows(Throwable.class, () -> { NestedStruct n = NestedStruct.readFrom(reader); });
            }
        }
    }
}

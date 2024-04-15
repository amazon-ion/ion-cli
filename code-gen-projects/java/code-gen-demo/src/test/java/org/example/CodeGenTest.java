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
         assertEquals("hello", s.getA(), "s.getA() should return \"hello\"");
         assertEquals(12, s.getB(), "s.getB() should return `12`");
         assertEquals(3, s.getC().getValue().size(), "s.getC().getValue() should return ArrayList fo size 3");
         assertEquals(10e2, s.getD(), "s.getD() should return `10e2`");
    }

    @Test void getterAndSetterTestForNestedStruct() {
         NestedStruct n = new NestedStruct("hello", 12, new AnonymousType1(false));
         assertEquals("hello", n.getA(), "n.getA() should return \"hello\"");
         assertEquals(12, n.getB(), "n.getB() should return `12`");
         assertEquals(false, n.getC().getD(), "n.getC().getD() should return `false`");
    }

    @Test void roundtripTestForStructWithFields() throws IOException {
        File f = new File("./../../input/struct_with_fields.ion");
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
        }
        assertEquals(ionLoader.load(f), ionLoader.load(out.toByteArray()));
    }

    @Test void roundtripTestForNestedStruct() throws IOException {
        File f = new File("./../../input/nested_struct.ion");
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
        }
        assertEquals(ionLoader.load(f), ionLoader.load(out.toByteArray()));
    }
}

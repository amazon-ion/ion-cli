package org.example;

import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;
import java.util.ArrayList;

class CodeGenTest {
    // TODO: Add roundtrip tests after read-write APIs are supported for Java code generation
    @Test void getterAndSetterTestForStructWithFields() {
         ArrayList<String> a = new ArrayList<String>();
         a.add("foo");
         a.add("bar");
         a.add("baz");
         StructWithFields s = new StructWithFields("hello", 12, new AnonymousType1(a));
         assertEquals("hello", s.getA(), "s.getA() should return \"hello\"");
         assertEquals(12, s.getB(), "s.getB() should return `12`");
         assertEquals(3, s.getC().getValue().size(), "s.getC().getValue() should return ArrayList fo size 3");
    }

    @Test void getterAndSetterTestForNestedStruct() {
             NestedStruct n = new NestedStruct("hello", 12, new AnonymousType2(false));
             assertEquals("hello", n.getA(), "n.getA() should return \"hello\"");
             assertEquals(12, n.getB(), "n.getB() should return `12`");
             assertEquals(false, n.getC().getD(), "n.getC().getD() should return `false`");
        }
}

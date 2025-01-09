import { readFileSync } from 'fs';
import { makeReader, makeWriter, Writer } from 'ion-js';
import { Decimal } from 'decimal.js';
import { IonTimestamp } from '../src/serializers/timestamp';
import { IonSymbol } from '../src/serializers/symbol';

describe('Ion Roundtrip Tests', () => {
  const readIonFile = (path: string) => {
    const data = readFileSync(path);
    const reader = makeReader(data);
    return reader;
  };

  describe('Good Input Tests', () => {
    test('handles all Ion types correctly', () => {
      // Test data covering all Ion types
      const testData = {
        nullValue: null,
        boolValue: true,
        intValue: BigInt("9223372036854775807"),
        floatValue: 123.456,
        decimalValue: new Decimal("123.456789"),
        timestampValue: new IonTimestamp("2023-04-01T12:00:00.000Z"),
        stringValue: "test string",
        symbolValue: new IonSymbol("test_symbol"),
        blobValue: new Uint8Array([1, 2, 3]),
        clobValue: "text/plain",
        listValue: [1, 2, 3],
        structValue: { key: "value" }
      };

      // Serialize
      const writer = makeWriter();
      writer.writeValues(testData);
      const serialized = writer.getBytes();

      // Deserialize
      const reader = makeReader(serialized);
      const deserialized = reader.next();

      // Compare
      expect(deserialized).toEqual(testData);
    });

    test('handles nested structures', () => {
      const complexData = {
        struct: {
          list: [1, "two", { three: 3 }],
          nested: {
            deep: {
              value: "nested"
            }
          }
        }
      };

      const writer = makeWriter();
      writer.writeValues(complexData);
      const serialized = writer.getBytes();

      const reader = makeReader(serialized);
      const deserialized = reader.next();

      expect(deserialized).toEqual(complexData);
    });
  });

  describe('Bad Input Tests', () => {
    test('rejects invalid timestamps', () => {
      expect(() => {
        new IonTimestamp("invalid-date");
      }).toThrow();
    });

    test('rejects invalid decimals', () => {
      expect(() => {
        new Decimal("not-a-number");
      }).toThrow();
    });

    test('handles null type mismatches', () => {
      const writer = makeWriter();
      writer.writeNull("string");
      const serialized = writer.getBytes();

      const reader = makeReader(serialized);
      const value = reader.next();
      
      expect(value).toBeNull();
      expect(reader.typeAnnotation()).toBe("string");
    });
  });
}); 
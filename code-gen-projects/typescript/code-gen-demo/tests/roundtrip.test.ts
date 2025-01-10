import { readFileSync } from 'fs';
import { makeReader, makeWriter } from 'ion-js';
import path from 'path';

// Import all generated types (these will be available after code generation)
import * as generated from '@generated/index';

describe('Ion TypeScript Code Generation Tests', () => {
  const ION_INPUT = process.env.ION_INPUT || '../../input';
  
  const readIonFile = (filePath: string) => {
    const fullPath = path.join(ION_INPUT, filePath);
    return readFileSync(fullPath);
  };

  describe('Good Input Tests', () => {
    test('struct_with_fields roundtrip', () => {
      const data = readIonFile('good/struct_with_fields.ion');
      const reader = makeReader(data);
      
      // Read from Ion
      const value = generated.StructWithFields.fromIon(reader);
      expect(value).toBeDefined();
      
      // Write back to Ion
      const writer = makeWriter();
      value.toIon(writer);
      const serialized = writer.getBytes();
      
      // Read again and compare
      const newReader = makeReader(serialized);
      const newValue = generated.StructWithFields.fromIon(newReader);
      expect(newValue).toEqual(value);
    });

    test('sequence roundtrip', () => {
      const data = readIonFile('good/sequence.ion');
      const reader = makeReader(data);
      
      // Read from Ion
      const value = generated.Sequence.fromIon(reader);
      expect(value).toBeDefined();
      
      // Write back to Ion
      const writer = makeWriter();
      value.toIon(writer);
      const serialized = writer.getBytes();
      
      // Read again and compare
      const newReader = makeReader(serialized);
      const newValue = generated.Sequence.fromIon(newReader);
      expect(newValue).toEqual(value);
    });

    test('enum_type roundtrip', () => {
      const data = readIonFile('good/enum_type.ion');
      const reader = makeReader(data);
      
      // Read from Ion
      const value = generated.EnumType.fromIon(reader);
      expect(value).toBeDefined();
      
      // Write back to Ion
      const writer = makeWriter();
      value.toIon(writer);
      const serialized = writer.getBytes();
      
      // Read again and compare
      const newReader = makeReader(serialized);
      const newValue = generated.EnumType.fromIon(newReader);
      expect(newValue).toEqual(value);
    });
  });

  describe('Bad Input Tests', () => {
    test('invalid struct_with_fields', () => {
      const data = readIonFile('bad/struct_with_fields.ion');
      const reader = makeReader(data);
      
      expect(() => {
        generated.StructWithFields.fromIon(reader);
      }).toThrow();
    });

    test('invalid sequence', () => {
      const data = readIonFile('bad/sequence.ion');
      const reader = makeReader(data);
      
      expect(() => {
        generated.Sequence.fromIon(reader);
      }).toThrow();
    });

    test('invalid enum_type', () => {
      const data = readIonFile('bad/enum_type.ion');
      const reader = makeReader(data);
      
      expect(() => {
        generated.EnumType.fromIon(reader);
      }).toThrow();
    });
  });
}); 
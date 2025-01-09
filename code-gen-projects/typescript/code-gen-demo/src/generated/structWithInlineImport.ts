import * as ion from 'ion-js';
import { IonSerializable, StructType } from './ion_generated_code';



export interface StructWithInlineImport extends IonSerializable {
}

/**
 * Type guard for StructWithInlineImport
 * @param value - Value to check
 * @returns True if value is StructWithInlineImport
 */
export function isStructWithInlineImport(value: any): value is StructWithInlineImport {
    if (typeof value !== 'object' || value === null) return false;
    return true;
}

/**
 * Implementation class for StructWithInlineImport
 */
export class StructWithInlineImportImpl implements StructWithInlineImport {

    constructor() {
    }

    /**
     * Serialize to Ion format
     * @returns Serialized bytes
     */
    public toIon(): any {
        const writer = ion.makeTextWriter();
        writer.stepIn(StructType);
        writer.stepOut();
        return writer.getBytes();
    }

    /**
     * Deserialize from Ion format
     * @param reader - Ion reader
     * @returns Deserialized StructWithInlineImport
     */
    public static fromIon(reader: ion.Reader): StructWithInlineImport {
        const result = new StructWithInlineImportImpl();
        reader.stepIn();
        while (reader.next() !== null) {
            const fieldName = reader.fieldName();
            switch (fieldName) {
                default:
                    throw new Error(`Unknown field: ${fieldName}`);
            }
        }
        reader.stepOut();
        return result;
    }
} 
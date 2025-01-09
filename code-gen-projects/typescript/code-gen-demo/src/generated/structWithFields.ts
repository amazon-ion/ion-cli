import * as ion from 'ion-js';
import { IonSerializable, StructType } from './ion_generated_code';



export interface StructWithFields extends IonSerializable {
}

/**
 * Type guard for StructWithFields
 * @param value - Value to check
 * @returns True if value is StructWithFields
 */
export function isStructWithFields(value: any): value is StructWithFields {
    if (typeof value !== 'object' || value === null) return false;
    return true;
}

/**
 * Implementation class for StructWithFields
 */
export class StructWithFieldsImpl implements StructWithFields {

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
     * @returns Deserialized StructWithFields
     */
    public static fromIon(reader: ion.Reader): StructWithFields {
        const result = new StructWithFieldsImpl();
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
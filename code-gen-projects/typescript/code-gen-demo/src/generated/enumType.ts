import * as ion from 'ion-js';
import { IonSerializable } from './ion_generated_code';



export enum EnumType {FOO_BAR_BAZ = "FooBarBaz", BAR = "bar", BAZ = "baz", FOO = "foo"
}

/**
 * Type guard for EnumType
 * @param value - Value to check
 * @returns True if value is EnumType
 */
export function isEnumType(value: any): value is EnumType {
    return Object.values(EnumType).includes(value);
}

/**
 * Implementation class for EnumType serialization
 */
export class EnumTypeImpl implements IonSerializable {
    private value: EnumType;

    constructor(value: EnumType) {
        this.value = value;
    }

    /**
     * Serialize to Ion format
     * @returns Serialized bytes
     */
    public toIon(): any {
        const writer = ion.makeTextWriter();
        writer.writeSymbol(this.value);
        return writer.getBytes();
    }

    /**
     * Deserialize from Ion format
     * @param reader - Ion reader
     * @returns Deserialized EnumType
     * @throws Error if value is invalid
     */
    public static fromIon(reader: ion.Reader): EnumType {
        const value = reader.stringValue();
        if (!value || !isEnumType(value)) {
            throw new Error(`Invalid enum value for EnumType: ${value}`);
        }
        return value as EnumType;
    }
} 
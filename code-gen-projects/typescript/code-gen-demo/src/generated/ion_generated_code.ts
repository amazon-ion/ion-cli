import * as ion from 'ion-js';
import { Decimal } from 'decimal.js';

export interface IonSerializable {
    toIon(): any;
}

export interface IonSymbol {
    text: string;
    sid?: number;
    local_sid?: number;
}

export interface IonTimestamp {
    value: Date;
}

export interface IonDecimal {
    value: string;
    coefficient: bigint;
    exponent: number;
}

// Re-export the Ion types we need
export const { LIST: ListType, STRUCT: StructType } = ion.IonTypes; 
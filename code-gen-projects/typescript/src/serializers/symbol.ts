import { Writer, Reader } from 'ion-js';

export class IonSymbol {
  private value: string;
  private annotations: string[];

  constructor(value: string, annotations: string[] = []) {
    this.value = value;
    this.annotations = annotations;
  }

  public getValue(): string {
    return this.value;
  }

  public getAnnotations(): string[] {
    return [...this.annotations];
  }

  public hasAnnotation(annotation: string): boolean {
    return this.annotations.includes(annotation);
  }

  public toString(): string {
    if (this.annotations.length > 0) {
      return `${this.annotations.join("::")}::${this.value}`;
    }
    return this.value;
  }

  public equals(other: IonSymbol): boolean {
    return this.value === other.value &&
           this.annotations.length === other.annotations.length &&
           this.annotations.every((ann, idx) => ann === other.annotations[idx]);
  }

  public static fromString(value: string): IonSymbol {
    const parts = value.split("::");
    const symbolValue = parts.pop() || "";
    return new IonSymbol(symbolValue, parts);
  }

  public writeToWriter(writer: Writer): void {
    if (this.annotations.length > 0) {
      writer.setAnnotations(this.annotations);
    }
    writer.writeSymbol(this.value);
  }

  public static fromReader(reader: Reader): IonSymbol {
    const annotations = reader.getAnnotations();
    const value = reader.stringValue();
    return new IonSymbol(value || "", annotations);
  }

  public toJSON(): string {
    return this.toString();
  }

  public static isSymbol(value: any): value is IonSymbol {
    return value instanceof IonSymbol;
  }
} 
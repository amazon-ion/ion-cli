import { Timestamp } from 'ion-js';

export class IonTimestamp {
  private timestamp: Timestamp;
  
  constructor(value: string | Date) {
    if (value instanceof Date) {
      this.timestamp = Timestamp.parse(value.toISOString());
    } else {
      try {
        this.timestamp = Timestamp.parse(value);
      } catch (e) {
        throw new Error(`Invalid timestamp format: ${value}`);
      }
    }
  }

  public getYear(): number {
    return this.timestamp.getYear();
  }

  public getMonth(): number {
    return this.timestamp.getMonth();
  }

  public getDay(): number {
    return this.timestamp.getDay();
  }

  public getHour(): number {
    return this.timestamp.getHour();
  }

  public getMinute(): number {
    return this.timestamp.getMinute();
  }

  public getSecond(): number {
    return this.timestamp.getSecond();
  }

  public getFractionalSecond(): number | undefined {
    return this.timestamp.getFractionalSecond();
  }

  public getPrecision(): Timestamp.Precision {
    return this.timestamp.getPrecision();
  }

  public getLocalOffset(): number | undefined {
    return this.timestamp.getLocalOffset();
  }

  public toDate(): Date {
    return this.timestamp.toDate();
  }

  public toString(): string {
    return this.timestamp.toString();
  }

  public static fromIon(timestamp: Timestamp): IonTimestamp {
    return new IonTimestamp(timestamp.toString());
  }

  public toIon(): Timestamp {
    return this.timestamp;
  }

  public equals(other: IonTimestamp): boolean {
    return this.timestamp.equals(other.timestamp);
  }
} 
# TypeScript Code Generation Project

This directory contains a TypeScript project that demonstrates code generation using `ion-cli` with TypeScript as the target language.

## Project Structure

```
typescript/
├── src/
│   ├── models/        # Generated TypeScript interfaces
│   ├── serializers/   # Ion serialization code
│   └── validators/    # Schema validation code
├── tests/
│   ├── good/         # Valid test cases
│   └── bad/          # Invalid test cases
├── package.json
└── tsconfig.json
```

## Build Process

The TypeScript code generation is integrated into the build process using npm scripts. The build process:

1. Checks for `ion-cli` availability
2. Generates TypeScript code from schemas
3. Compiles TypeScript to JavaScript
4. Runs tests

### NPM Scripts

```json
{
  "scripts": {
    "generate": "ion-cli generate -l typescript -d ../../schema -o ./src/models",
    "build": "tsc",
    "test": "jest",
    "clean": "rm -rf ./src/models/*"
  }
}
```

### Environment Setup

1. Install ion-cli:
   ```bash
   brew install ion-cli
   # or
   cargo install ion-cli
   ```

2. Set up environment:
   ```bash
   export ION_CLI=/path/to/ion-cli  # Optional, defaults to 'ion'
   ```

## Testing

The project includes comprehensive tests for the generated code:

### Unit Tests
- Type guard validation
- Serialization/deserialization
- Null value handling
- Type annotation preservation

### Integration Tests
- Roundtrip testing with good/bad inputs
- Schema validation
- Error handling

### Running Tests

```bash
npm test
```

## Type System

The generated TypeScript code follows these principles:

1. **Null Safety**
   - Explicit null handling
   - Optional type support
   - Undefined vs null distinction

2. **Type Guards**
   - Runtime type checking
   - Custom validation rules
   - Schema constraint validation

3. **Serialization**
   - Binary format support
   - Text format support
   - Type annotation preservation

## Ion Type Mappings

| Ion Type | TypeScript Type |
|----------|----------------|
| null     | null           |
| bool     | boolean        |
| int      | number/bigint  |
| float    | number        |
| decimal  | Decimal       |
| timestamp| Date          |
| string   | string        |
| symbol   | Symbol        |
| blob     | Uint8Array    |
| clob     | string        |
| struct   | interface     |
| list     | Array         |
| sexp     | Array         |

## Error Handling

The generated code includes comprehensive error handling:

- Schema validation errors
- Type conversion errors
- Serialization errors
- Runtime validation errors 
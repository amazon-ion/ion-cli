# TypeScript Code Generation Demo

This project demonstrates code generation using `ion-cli` with TypeScript as the target language. It uses the schema files from the parent directory and tests the generated code against both good and bad input files.

## Project Structure

```
code-gen-demo/
├── src/
│   └── generated/     # Generated TypeScript code from schemas
├── tests/
│   └── roundtrip.test.ts  # Roundtrip tests for generated code
├── package.json
└── tsconfig.json
```

## Prerequisites

1. Install ion-cli:
   ```bash
   brew install ion-cli
   # or
   cargo install ion-cli
   ```

2. Set up environment:
   ```bash
   export ION_CLI=/path/to/ion-cli  # Optional, defaults to 'ion'
   export ION_INPUT=/path/to/input  # Required for tests
   ```

## Build Process

The build process is integrated with npm scripts:

1. `npm run generate` - Generates TypeScript code from schemas
2. `npm run build` - Compiles TypeScript to JavaScript
3. `npm test` - Runs the test suite

## Running Tests

The tests verify that the generated code can:
- Read Ion data into TypeScript objects
- Write TypeScript objects back to Ion format
- Handle both valid and invalid input correctly

To run the tests:

```bash
# From the code-gen-demo directory
ION_INPUT=../../input npm test
```

## Test Cases

1. Good Input Tests:
   - Struct with fields
   - Sequences
   - Enum types
   - Nested structures
   - Type annotations

2. Bad Input Tests:
   - Invalid struct fields
   - Invalid sequence elements
   - Invalid enum values
   - Type mismatches

## Generated Code Features

The generated TypeScript code includes:
- Type-safe interfaces
- Runtime type guards
- Ion serialization/deserialization
- Null safety
- Type annotations support 
# Ion Code Generator User Guide

This guide introduces code generation using the `ion-cli` tool. The `generate` subcommand in `ion-cli` takes Ion Schema
Language (ISL) files as input and produces code based on the types defined in these schemas for both Java and Rust
programming languages.

This guide assumes the reader has a basic understanding of
the [Ion Schema Language](https://amazon-ion.github.io/ion-schema/docs/isl-2-0/spec).

This guide covers

1. [Installing ion-cli](#installing-ion-cli)
2. [Defining a Data Model](#defining-a-data-model)
3. [Running the generator](#running-the-generator)
4. [Using the generated code in a Java project](#using-the-generated-code-in-a-java-project)
5. [Using the generated code in a Rust project](#using-the-generated-code-in-a-rust-project)

## Installing `ion-cli`

### Install using `brew` (Mac only)

The easiest way to install the `ion-cli` is via [Homebrew](https://brew.sh/).
Once the `brew` command is available, run:

```sh
brew tap amazon-ion/ion-cli
brew install ion-cli
```

### Install using `cargo`

The `ion-cli` can also be installed by using Rust's package manager, `cargo`. If you don't already have `cargo`, you can
install it by visiting [rustup.rs](https://rustup.rs/).
To install `ion-cli`, run the following command:

```sh
cargo install ion-cli
```

## Defining a Data Model

The Ion code generator supports a variety of data types, both abstract and concrete

* Integers
* Strings
* Booleans
* Floats
* [Records](https://en.wikipedia.org/wiki/Record_(computer_science))
* [Lists](https://en.wikipedia.org/wiki/List_(abstract_data_type))
* [Enumerations](https://en.wikipedia.org/wiki/Enumerated_type)
* Nominally distinct "[new types](https://doc.rust-lang.org/rust-by-example/generics/new_types.html)"

### Defining a record type

A record type is defined by a type definition with a closed `fields` constraint.

**Example**

```
$ion_schema_2_0

type::{
    name: customer,
    fields: closed::{
      first_name: string,
      last_name: string,
      id: int,
    }
}
```

### Using Lists

The Ion code generator supports homogeneous lists. To use a homogenous list as a field type, you can specify it inline
using the `element` constraint.

To define a new type that is a homogeneous list, create a top-level type with an `element` constraint.

**Example**

```
$ion_schema_2_0

type::{
  name: product_tags,
  type: list,
  element: string,
}

type::{
  name: product_summary,
  fields: closed::{
    product_id: int,
    name: string,
    
    // The type of the `tags` field is the new type `product_tags`
    tags: product_tags,
    
    // The type of the `ratings` field is a list of integers
    ratings: { type: list, element: int },
  }
}
```

### Defining an enumeration

Enumerations are defined using `valid_values` constraint which contains only symbol values.

```
$ion_schema_2_0
type::{
   name: fruits,
   valid_values: [apple, banana, strawberry]
}
```

### Special considerations for inline types

Inline types in Ion Schema do not have a corresponding name for it. Here’s how the inline types are defined in ISL:

```
<INLINE_TYPE_DEFINITION> ::= { <CONSTRAINT>... }
```

For more information on inline types in Ion Schema,
see [Type Definitions](https://amazon-ion.github.io/ion-schema/docs/isl-2-0/spec#type-definitions).

Since an inline type does not have a name attached to it, code generator interprets the name of this inline type based
on where its placed in the schema. Below are few cases on how code generator interprets the name for the generated data
model for an inline type in ISL:

##### Inline type within `fields` constraint

If the inline type is defined inside a `fields` constraint then code generator uses the field name to generate the
inline type.
Inline types in a `fields` constraint should be named after their respective field names.
e.g.  **ISL:**

```
$ion_schema_2_0
type::{
   name: Foo,
  fields: closed::{
     // The inline type will be named `bar`, corresponding to the field name
     bar: { fields: { baz: string } }
  }
}
```

#### Inline type within `element` constraint:

If the inline type is defined inside `element` constraint then code generator gives it a static name `Element` which
will be used by its parent typed list.
e.g. **ISL:**

```
$ion_schema_2_0
type::{
   name: Foo,
   // The inline type here will be called "element"
   element: { fields: { bar: string } },
   type: list
}
```

## Running the generator

Use the `generate` subcommand with the following syntax:

```sh
ion -X generate [OPTIONS] --language <language> --authority <directory>
```

Required options:

* `--language` or `-l`: Specify the target programming language (java or rust)
* `-A`, `—authority`: The root(s) of the file system authority(s). (For more information what is an authority,
  see [Ion Schema Specification](https://amazon-ion.github.io/ion-schema/docs/isl-1-0/spec#schema-authorities))
* `--namespace` or `-n`: Provide a namespace for generated Java code (e.g., `org.example`)

Additional options:

* `--output` or `-o`: Specify the output directory for generated code (default: current directory)

Example:

```sh
ion -X generate -l java -n org.example -A ./schema -o ./generated/java
```

If you are looking to run the code generator at build time, follow this guide on adding code generation
to [build process in Java](#adding-to-the-build-process)
and [build process in Rust](#adding-to-the-build-process-1).

This repository also contains [examples](https://github.com/amazon-ion/ion-cli/tree/main/code-gen-projects) of how to
use the code generator in Java and Rust projects.

## Using the generated code in a Java project

### Adding to the build process

To generate code as part of the build process of a project, define a Gradle build task inside `build.gradle.kts` or
`build.gradle` .
The generated code requires a dependency to `ion-java` version `1.11.9` .
Following is a sample build task defined in `build.gradle.kts` that you can add in an existing Gradle project to
generate code for your schemas:

```kotlin
val ionSchemaSourceCodeDir = "YOUR_SOURCE_SCHEMA_DIRECTORY"
val generatedIonSchemaModelDir = "${layout.buildDirectory.get()}/generated/java"

sourceSets {
    main {
        java.srcDir(generatedIonSchemaModelDir)
    }
}


tasks {
    val ionCodegen = create<Exec>("ionCodegen") {
        inputs.files(ionSchemaSourceCodeDir)
        outputs.file(generatedIonSchemaModelDir)
        
        val ionCli = System.getenv("ION_CLI") ?: "ion"

        commandLine(ionCli)
            .args(
                "-X", "generate",
                "-l", "java",
                "-n", "NAMESPACE_FOR_GENERATED_CODE",
                "-A", ionSchemaSourceCodeDir,
                "-o", generatedIonSchemaModelDir,
            )
            .workingDir(rootProject.projectDir)
    }

    withType<JavaCompile> {
        dependsOn(ionCodegen)
    }
}
```

This task performs following steps:

* Gets the executable path for ion-cli through an environment variable `ION_CLI`. If the environment variable is not set
  then it uses the local executable named `ion`.
* Sets the schema authority as provided which will be used by code generator to generate code for the schema files
  inside that authority.
* Sets the path to output directory where the code will be generated and sets it as source directory.
* It runs the code generator with the set schema directory and a namespace where the code will be generated.

### About the generated code

Each generated *record* includes getters for all fields, a builder, a `writeTo(IonWriter)` method, and a static
`readFrom(IonReader)` method. When a field is one of the (Java) primitive types, if it is an optional field, the
generator will render a boxed primitive instead of the primitive type.

All other generated types have a `writeTo(IonWriter)` method, a static `readFrom(IonReader)` method, and (when
applicable) a public, single-argument constructor.

### Read a generated type from an `IonReader`

Generated type has a `readFrom` method which can be used to read Ion data using an `IonReader` and initialize the
generated type with the given Ion data. *(Generated code here depends on `ion-java` version `1.11.9`)*

```java
// asssume that the generated type is `Foo`
IonReaderBuilder readerBuilder = IonReaderBuilder.standard();
try (IonReader reader = readerBuilder.build(bufferedStream)) {
    reader.next();
    Foo foo = Foo.readFrom(reader);
}
```

### Write a generated type to an `IonWriter`

Generated type has a `writeTo` method which can be used to write the type as Ion using an `IonWriter` . *(Generated code
here depends on `ion-java` version `1.11.9`)*

```java
// asssume that the generated type is initialized as `foo`
IonWriter writer = b.build(out);
foo.writeTo(writer);
writer.close();
```

## Using the generated code in a Rust project

### Adding to the build process

To generate code as part of the build process of a Cargo project, define a cargo build script in `build.rs`.
The generated code requires a dependency to `ion-rs` version `1.0.0-rc.2` .
Following is sample build script you can add in your existing Cargo project to generate code using `ion-cli`:

```rust
fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    // Invokes the ion-cli executable using environment variable ION_CLI if present, 
    // otherwise uses local executable named `ion`
    let ion_cli = env::var("ION_CLI").unwrap_or("ion".to_string());

    let mut cmd = std::process::Command::new(ion_cli);
    cmd.arg("-X")
        .arg("generate")
        .arg("-l")
        .arg("rust")
        .arg("-A")
        .arg("YOUR_SOURCE_SCHEMA_DIRECTORY")
        .arg("-o")
        .arg(&out_dir);

    println!("cargo:warn=Running: {:?}", cmd);

    let output = cmd.output().expect("failed to execute process");

    io::stdout().write_all(&output.stdout).unwrap();
    io::stderr().write_all(&output.stderr).unwrap();

    assert!(output.status.success());
}
```

This task performs following steps:

* Gets the executable path for ion-cli through an environment variable `ION_CLI`. If the environment variable is not set
  then it uses the local executable named ion.
* Sets the schema directory as provided which will be used by generator to generate code for the schema files inside it.
* Sets the path to output directory where the code will be generated (e.g. `OUT_DIR`).
* It runs the code generator with the set schema directory and a namespace where the code will be generated.

### About the generated code

All the generated types include getters, a public constructor with `new`, a `writeTo(Writer)` method, and
`readFrom(Reader)` method.

If a type is defined inline in the ISL definition of another type, the generated code will include a module with the
same name as the outer type (although the module name will be snake case instead of Pascal case) which contains the
types defined within the outer types.

### Read a generated type from an `Reader`

Generated type has a `readFrom` method which can be used to read Ion data using `Reader` and initialize the generated
type with the given Ion data. *(Generated code here depends on `ion-rs` version `1.0.0-rc.2`)*

```rust
let mut reader: Reader = ReaderBuilder::new().build(ION_DATA) ?;
reader.next() ?;
let foo: Foo = Foo::read_from( & mut reader) ?;
```

### Write a generated type to an `Writer`

Generated type has a `writeTo` method which can be used to write the type as Ion using `Writer` . *(Generated code here
depends on `ion-rs` version `1.0.0-rc.2`)*

```rust
let mut text_writer = TextWriterBuilder::default ().build( & mut buffer) ?;
foo.write_to( & mut text_writer) ?;
text_writer.flush() ?;
```

## Appendix A – Built-in ISL types and corresponding generated types

✅ Supported
🟡 Planned to support
❌ No plans to support (yet)

| ISL Type	    | Java Type	                     | Rust Type	              | Notes	                                                                   |
|--------------|--------------------------------|-------------------------|--------------------------------------------------------------------------|
| `nothing`	   | ❌	                             | ❌	                      | By definition, it is not possible to construct an instance of `nothing`	 |
| `bool`	      | ✅ `Boolean` / `boolean`	       | ✅ `bool`	               | 	                                                                        |
| `int`	       | ✅ `Integer` / `int`	           | ✅ `i64`	                | 	                                                                        |
| `float`	     | ✅ `Double` / `double`	         | ✅ `f64`	                | 	                                                                        |
| `decimal`	   | 🟡 `com.amazon.ion.Decimal`	   | 🟡 `ion_rs::Decimal`	   | 	                                                                        |
| `timestamp`	 | 🟡 `com.amazon.ion.Timestamp`	 | 🟡 `ion_rs::Timestamp`	 | 	                                                                        |
| `string`	    | ✅ `String`	                    | ✅ `String`	             | 	                                                                        |
| `symbol`	    | ✅ `String`	                    | ✅ `String`	             | 	                                                                        |
| `clob`	      | 🟡 `byte[]`	                   | 🟡 `Vec<u8>`	           | 	                                                                        |
| `blob`	      | 🟡 `byte[]`	                   | 🟡 `Vec<u8>`	           | 	                                                                        |
| `list`	      | ✅ `java.util.List`	            | ✅ `std::vec::Vec`	      | See [Using Lists](#using-lists)	                                         |
| `sexp`	      | ✅ `java.util.List`	            | ✅ `std::vec::Vec`	      | See [Using Lists](#using-lists)	                                         |
| `struct`	    | ✅	                             | ✅	                      | See [Defining a record type](#defining-a-record-type)	                   |
| `document`	  | 🟡	                            | 🟡	                     | 	                                                                        |
| `lob`	       | 🟡	                            | 🟡	                     | 	                                                                        |
| `text`	      | 🟡	                            | 🟡	                     | 	                                                                        |
| `number`	    | 🟡	                            | 🟡	                     | 	                                                                        |
| `any`	       | ❌	                             | ❌	                      | 	                                                                        |

The built-in types starting with `$` are not currently planned to be supported.
The `struct` built-in type may have additional support for generating maps with string keys in future.

## Appendix B - Examples of generated code based on ISL type definitions in Java

Here are some examples on generated code for ISL type definitions:

*Note: generated code here was trimmed to represent only the portion of code necessary for this example. Each generated
data model will have its builder, getters, setters, readFrom(which read Ion data to the model) and writeTo(which writes
the model as Ion data) are defined.*

#### Generating classes

**Ion Schema:**

```
type::{
    // this will be used as the name of the generated class
    name: person,
    // currently code generation doesn't allow open ended types,
    // so for defining a `type` constraint is required
    type: struct, 
    fields:{
        first_name: string,
        last_name: string,
        age: int,
    }
}
```

**Generated Code in Java:**

```java
class Person {
    private Integer age;
    private String lastName;
    private String firstName;
    private Person() {}

    public String getFirstName() {
        return this.firstName;
    }
    
    public Integer getAge() {
        return this.age;
    }
    
    public String getLastName() {
        return this.lastName;
    }

    public void setFirstName(String firstName) {
        this.firstName = firstName;
        return;
    }

    public void setAge(Integer age) {
        this.age = age;
        return;
    }

    public void setLastName(String lastName) {
        this.lastName = lastName;
        return;
    }

    public static class Builder {
        // ...
    }
    
    /**
     * Reads a Person from an {@link IonReader}.
     *
     * This method does not advance the reader at the current level.
     * The caller is responsible for positioning the reader on the value to read.
     */
    public static Person readFrom(IonReader reader) {
        // ...
    }
    
    /**
     * Writes a Person as Ion from an {@link IonWriter}.
     *
     * This method does not close the writer after writing is complete.
     * The caller is responsible for closing the stream associated with the writer.
     * This method skips writing a field when it's null.
     */
    public void writeTo(IonWriter writer) throws IOException {
        // ...
    }
}
```

#### Generating nested classes

**Ion Schema:**

```
type::{
    // this will be used as the name of the generated class
    name: customer,
    // currently code generation doesn't allow open ended types,
    // so for defining a `type` constraint is required
    type: struct, 
    fields:{
        name: string,
        // this nested struct will be generated as nested class in the generated code
        address:{
            type: struct,
            fields: {
                street: string,
                city: string,
                state: string,
                postal_code: string
           }
         }
    }
}
```

**Generated code in Java:**

```java
 class Customer {
    private org.example.Customer.Address address;
    private String name;
     ...
    static class Address {
        private String state;
        private String city;
        private String street;
        private String postalCode;
        ...
    }
}
```

#### Generating enum

**Ion Schema:**

```
type::{
    // this will be used as the name of the generated enum
    name: fruits,
    type: symbol,
    // currently only symbol values are supported `valid_values` 
    // for enum generation
    valid_values: [apple, banana, mango]
}
```

**Generated code in Java:**

```java
public  enum Fruits {
    APPLE("apple"),
    BANANA("banana"),
    MANGO("mango"),
    ;
     ...
     
    public static Fruits readFrom(IonReader reader) {
        // ...
    }
    
    public void writeTo(IonWriter writer) throws IOException {
        // ...
    }
}
```

## Appendix C - Supported Schema Features

Currently, code generation supports basic ISL constraints:

* Supported constraints: type, element, fields (including inline type definitions), valid_values (with symbol values)
* Supported data models: class, enum, typed list, scalar
* Supports imports
* Supports variably occurring types (`optional` or `required`) within `fields` constraint

Limitations:

* Heterogeneous lists not supported (e.g., `{type: list}` without an `element` constraint)
* Discriminated union types (e.g., types with `one_of`) are not supported
* `struct`s with arbitrary field names are not supported
* Optional fields are not supported when targeting
  Rust ([ion-cli#102](https://github.com/amazon-ion/ion-cli/issues/102))
* Built-in Ion Schema types `timestamp`, `decimal`, `text`, `lob`, `number`, and `document` are not yet
  supported ([ion-cli#173](https://github.com/amazon-ion/ion-cli/issues/173)


## `ion-cli`
This repository is home to the `ion` command line tool, which provides subcommands
for working with [the Ion data format](https://amzn.github.io/ion-docs/docs/spec.html).

## Examples

These examples use the `.ion` file extension for text Ion and the `.10n` file
extension for binary Ion. This is simply a convention; the tool does not
evaluate the file extension.

Unless otherwise noted, these commands can accept any Ion format as input.

### Converting between formats with `dump`

Convert Ion text (or JSON) to Ion binary:
```shell
ion dump --format binary my_file.ion
```

Convert Ion binary to generously-spaced, human-friendly text:
```shell
ion dump --format pretty my_file.10n
```

Convert Ion binary to minimally-spaced, compact text:
```shell
ion dump --format text my_file.10n
```

### Analyzing binary Ion file encodings with `inspect`

The `beta inspect` command can display the hex bytes of a binary Ion file alongside
the equivalent text Ion for easier analysis.

```shell
# Write some text Ion to a file
echo '{foo: null, bar: true, baz: [1, 2, 3]}' > my_file.ion

# Convert the text Ion to binary Ion
ion dump --format binary my_file.ion > my_file.10n

# Show the binary encoding alongside its equivalent text 
ion beta inspect my_file.10n

---------------------------------------------------------------------------
 Offset   |  Length   |        Binary Ion        |         Text Ion
---------------------------------------------------------------------------
          |         4 | e0 01 00 ea              |  // Ion 1.0 Version Marker
        4 |         4 | ee 95 81 83              |  '$ion_symbol_table':: // $3::
        8 |        19 | de 91                    |  {
       10 |         1 | 86                       |    'imports': // $6:
       11 |         2 | 71 03                    |    $ion_symbol_table, // $3
       13 |         1 | 87                       |    'symbols': // $7:
       14 |        13 | bc                       |    [
       15 |         4 | 83 66 6f 6f              |       "foo",
       19 |         4 | 83 62 61 72              |       "bar",
       23 |         4 | 83 62 61 7a              |       "baz",
          |           |                          |    ],
          |           |                          |  }
       27 |        13 | dc                       |  {
       28 |         1 | 8a                       |    'foo': // $10:
       29 |         1 | 0f                       |     null,
       30 |         1 | 8b                       |    'bar': // $11:
       31 |         1 | 11                       |     true,
       32 |         1 | 8c                       |    'baz': // $12:
       33 |         7 | b6                       |    [
       34 |         2 | 21 01                    |       1,
       36 |         2 | 21 02                    |       2,
       38 |         2 | 21 03                    |       3,
          |           |                          |    ],
          |           |                          |  }
```

To skip to a particular offset in the stream, you can use the `--skip-bytes` flag:

```
ion beta inspect --skip-bytes 30 my_file.10n
---------------------------------------------------------------------------
 Offset   |  Length   |        Binary Ion        |         Text Ion
---------------------------------------------------------------------------
          |         4 | e0 01 00 ea              |  // Ion 1.0 Version Marker
          |           | ...                      |  // Skipped 23 bytes of user-level data
       27 |        13 | dc                       |  {
          |           | ...                      |    // Skipped 2 bytes of user-level data
       30 |         1 | 8b                       |    'bar': // $11:
       31 |         1 | 11                       |    true,
       32 |         1 | 8c                       |    'baz': // $12:
       33 |         7 | b6                       |    [
       34 |         2 | 21 01                    |       1,
       36 |         2 | 21 02                    |       2,
       38 |         2 | 21 03                    |       3,
          |           |                          |    ],
          |           |                          |  }
```

Notice that the text column adds comments indicating where data has been skipped.
Also, if the requested index is nested inside one or more containers, the beginnings
of those containers (along with their lengths and offsets) will still be included
in the output.

You can limit the amount of data that `inspect` displays by using the `--limit-bytes`
flag:

```shell
ion beta inspect --skip-bytes 30 --limit-bytes 2 my_file.10n
---------------------------------------------------------------------------
 Offset   |  Length   |        Binary Ion        |         Text Ion
---------------------------------------------------------------------------
          |         4 | e0 01 00 ea              |  // Ion 1.0 Version Marker
          |           | ...                      |  // Skipped 23 bytes of user-level data
       27 |        13 | dc                       |  {
          |           | ...                      |    // Skipped 2 bytes of user-level data
       30 |         1 | 8b                       |    'bar': // $11:
       31 |         1 | 11                       |    true,
          |           | ...                      |    // --limit-bytes reached, stepping out.
          |           |                          |  }
```

## Installation

The `ion-cli` is written in Rust. The easiest way to install it on your machine is
by using Rust's package manager, `cargo`. If you don't already have `cargo`, you
can install it by visiting [rustup.rs](https://rustup.rs/).

To install `ion-cli`, run the following command:

```shell
cargo install ion-cli
```

Then make sure that `~/.cargo/bin` is on your `$PATH`. You can confirm that it
has been installed successfully by running:

```shell
ion help
```

You should see output that resembles the following:

```
ion 0.4.0
The Ion Team <ion-team@amazon.com>

USAGE:
    ion <SUBCOMMAND>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

SUBCOMMANDS:
    beta    The 'beta' command is a namespace for commands whose interfaces are 
            not yet stable.
    dump    Prints Ion in the requested format
    help    Prints this message or the help of the given subcommand(s)
```

## Developer build instructions

1. Clone the repository:
   ```
   git clone https://github.com/amzn/ion-cli.git
   ```

2. Step into the newly created directory:
   ```
   cd ion-cli
   ```

3. Install Rust/Cargo [by visiting rustup.rs](https://rustup.rs/).

4. Build the `ion` tool:
   ```
   cargo install --path .
   ```
   This will put a copy of the `ion` executable in `~/.cargo/bin`.

5. Confirm that `~/.cargo/bin` is on your `$PATH`. `rustup` will probably take care of this for you.

6. Confirm that the executable is available by running:
   ```
   ion help
   ```

### Docker Instructions

1. Install Docker (see OS specific instructions on the [Docker website](https://docs.docker.com/get-docker/))
2. Clone the repository (recursive clone not necessary)
   ```
   git clone https://github.com/amzn/ion-cli.git
   ```
3. Step into the newly created directory
   ```
   cd ion-cli
   ```
4. Build and run the image
   ```
   # build the image
   docker build -t <IMAGE_NAME>:<TAG> .


   # run the CLI binary inside the Docker image
   docker run -it --rm [optional flags...] <IMAGE_NAME>:<TAG> ion <SUBCOMMAND>

   # examples:

   # build docker image with current release version
   docker build -t ion-cli:0.1.1 .

   # print the help message
   docker run -it --rm ion-cli:0.1.1 ion -V

   # mount current directory to /data volume and dump an ion file
   docker run -it --rm -v $PWD:/data ion-cli:0.1.1 ion dump /data/test.ion

   ```

## Security

See [CONTRIBUTING](CONTRIBUTING.md#security-issue-notifications) for more information.

## License

This project is licensed under the Apache-2.0 License.

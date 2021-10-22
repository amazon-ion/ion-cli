## `ion-cli`

_This package is considered experimental. It is under active/early development,
and the API is subject to change._

## Build Instructions

1. Clone the repository
   ```
   git clone --recursive https://github.com/amzn/ion-cli.git
   ```
   (If you had already cloned it, but the `ion-c` directory is missing or empty, run `git submodule update --init --recursive`.)

2. Step into the newly created directory
   ```
   cd ion-cli
   ```

3. Install Rust/Cargo [via `rustup`](https://rustup.rs/)

4. Build the `ion` tool
   ```
   cargo install --path .
   ```
   This will put a copy of the `ion` executable in `~/.cargo/bin`.

   **If this step fails:** You're likely missing one of `ion-c`'s dependencies. Make sure you have `cmake`, `gcc`, `g++` and `clang` installed. On Debian-based Linux distributions, the only required dependencies are `cmake` and `clang`.

5. Confirm that `~/.cargo/bin` is on your `$PATH`. `rustup` will probably take care of this for you.

6. Confirm that the executable is available
   ```
   ion help
   ```

### Docker Instructions

1. Clone the repository (recursive clone not necessary)
   ```
   git clone https://github.com/amzn/ion-cli.git
   ```
2. Step into the newly created directory
   ```
   cd ion-cli
   ```
3. Install Docker (see OS specific instructions on the [Docker website](https://docs.docker.com/get-docker/))
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

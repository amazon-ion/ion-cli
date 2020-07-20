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

**If this step fails:** You're likely missing one of `ion-c`'s dependencies. Make sure you have `cmake`, `gcc`, `g++`, and `libc++` installed.

5. Confirm that `~/.cargo/bin` is on your `$PATH`. `rustup` will probably take care of this for you.

7. Confirm that the executable is available
```
ion help
```

## Security

See [CONTRIBUTING](CONTRIBUTING.md#security-issue-notifications) for more information.

## License

This project is licensed under the Apache-2.0 License.

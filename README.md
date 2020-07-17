## `ion-cli`

_This package is considered experimental. It is under active/early development,
and the API is subject to change._

## Build Instructions

1. Clone the repository
```
git clone https://github.com/amzn/ion-cli.git
```

2. Step into the newly created directory
```
cd ion-cli
```

3. Run the following command to initialize all of the necessary git submodules
```
git submodule update --init --recursive
```

4. Install Rust/Cargo [via `rustup`](https://rustup.rs/)

5. Build the `ion` tool
```
cargo install --path .
```
This will put a copy of the `ion` executable in `~/.cargo/bin`.

6. Add `~/.cargo/bin` to your `$PATH`

7. Confirm that the executable is available
```
ion help
```

## Security

See [CONTRIBUTING](CONTRIBUTING.md#security-issue-notifications) for more information.

## License

This project is licensed under the Apache-2.0 License.

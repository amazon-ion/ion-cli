The packdump files are generated via the `synpack` command from the `gendata` example in `syntect`.

Check out `syntect` and then run:
```shell
cargo run --features=metadata --example gendata -- \
    synpack ion.sublime-syntax ion.newlines.packdump ion.nonewlines.packdump
```

`ion.sublime-syntax` is sourced from `partiql/partiql-rust-cli`.

# Cargo-image

[![Crates.io](https://img.shields.io/crates/v/cargo-image.svg)](https://crates.io/crates/cargo-image)
![maintenance-as-is](https://img.shields.io/badge/maintenance-as--is-yellow.svg)

An alternative to [`bootimage`](https://crates.io/crates/bootimage) that doesn't use `cargo-xbuild`.

Intended to be used with [`cargo-sysroot`](https://crates.io/crates/cargo-sysroot),
this tool will create an image bootable in QEMU for you, using the [`bootloader`](https://crates.io/crates/bootloader) crate.

## Details

Unlike `bootimage`, the standard `cargo-build` command is used to build your kernel.

It is expected that your `.cargo/config` be configured to pass `--sysroot` and have a proper `target`, such as by using `cargo-sysroot`.

## Prerequisite

* A nightly compiler.
* A `.cargo/config` setup to build your target.

An example `.cargo/config` might look like this. These are the absolute minimum settings required for `cargo-image` to work.

`cargo-sysroot` will automatically set this up for you, if you use it. `cargo-sysroot` is not a requirement.

```rust
[build]
target = "path/to/your/target/specification/json"
rustflags = [
    "--sysroot",
    "full/path/to/target/sysroot",
]
```

## FAQ

* Q: What about `bootimage`?
* A: 🤷. It didn't work for my needs, so I wrote my own.

## License

Licensed under either of

* Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
* MIT license
   ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

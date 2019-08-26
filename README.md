# Cargo-image

[![Crates.io](https://img.shields.io/crates/v/cargo-image.svg)](https://crates.io/crates/cargo-image)
![maintenance-as-is](https://img.shields.io/badge/maintenance-as--is-yellow.svg)

An alternative to [`bootimage`](https://crates.io/crates/bootimage) using [`cargo-sysroot`](https://crates.io/crates/cargo-sysroot).

The advantage of `cargo-sysroot` is that it's composable, eg other tools will work with it,
even if they don't know about it, because it sets up cargo so that the
normal commands like `cargo build` will work.

Like `bootimage`, this tool will combine your kernel with the
x86_64 [`bootloader`](https://crates.io/crates/bootloader) crate, so you can, well, boot it.

## Usage

In your project directory, simply run `cargo image`.
The output image will be located at `target/{your-triple}/debug/{your-binary-name}.bin`. Your binary name will usually be the name of your project.

## Details

The `bootloader` sysroot crates are compiled using `cargo sysroot`,
and `cargo sysroot` will be called before building your kernel, to ensure everything is up to date.

## Prerequisite

* A nightly compiler.
* A `.cargo/config` setup to build your target.
* `cargo-sysroot` v0.5.4 or later.
* `bootloader` v0.8.0. Older versions are untested and probably won't work.

## Limitations

* No attempt is made to follow the `.cargo/config` search path, eg this tool will not look in the parent directory like `cargo` would.
* Your kernel will only ever be built in Debug mode. This will change in the future.

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

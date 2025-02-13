# ffi-enum

Simply write and use `enum`s like rust native enums, freely passing through ffi.

[![Crates.io][crates-badge]][crates-url]
[![Docs.rs][docs-badge]][docs-url]
[![MIT licensed][mit-badge]][mit-url]
[![Build Status][actions-badge]][actions-url]

[crates-badge]: https://img.shields.io/crates/v/ffi-enum.svg
[crates-url]: https://crates.io/crates/ffi-enum
[docs-badge]: https://img.shields.io/docsrs/ffi-enum
[docs-url]: https://docs.rs/ffi-enum/latest/ffi_enum/
[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: https://github.com/AlseinX/ffi-enum/blob/master/LICENSE
[actions-badge]: https://github.com/AlseinX/ffi-enum/workflows/Check/badge.svg
[actions-url]: https://github.com/AlseinX/ffi-enum/actions?query=workflow%3ACI+branch%3Aslaveholder

## Why not using `#[repr(C)]` and `#[non_exhaustive]` ?

+ Rust's `#[repr(C)]` is not fully equal to a C-abi `enum`, and it is still an **undefined behavior** when an `enum` defined with `#[repr(C)]` recieves a value that is not listed in the definition of `enum`, from ffi.
+ `#[non_exhaustive]` is only designed for inter-crate compatibility, while the compiler might still optimize based on the assumption that the binary pattern of an `enum` value is always in its defined range.

## Why not existing alternatives?

The current alternatives mostly define complex DSL with function macros, which could be hard to read/maintain and not supported by `rustfmt` or `rust-analyzer`, and they do not support derive macros.

While this crate offers a nearly native experience by trying the best to fully mimic the behaviors of native `enums`, despite it requires non-exhaustive matching even inside the defining crate. FFi enums are defined with native rust enum item syntax with full support of formatting, code hint, and auto completion.

```rust
use ffi_enum::prelude::*;
use serde::{Deserialize, Serialize};

#[ffi_enum]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Animal {
    Cat,
    Dog,
}

fn main() {
    let json = serde_json::to_string(&Animal::Cat).unwrap();
    assert_eq!(json, "\"cat\"");
    let value: Animal = serde_json::from_str(&json).unwrap();
    assert_eq!(value, Animal::Cat);
    let json = serde_json::to_string(&Animal::from(100u8)).unwrap();
    assert_eq!(json, "\"<Unknown>\"");
    let value: Animal = serde_json::from_str(&json).unwrap();
    assert_eq!(value, Animal::UNKNOWN);
}
```

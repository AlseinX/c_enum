use proc_macro::TokenStream;
use syn::{parse_macro_input, Error};

mod delegate;
mod ffi_enum;
mod utils;

macro_rules! export_attribute {
    ($($(#[$attr:meta])* $i:ident),*$(,)?) => {$(
        $(#[$attr])*
        #[proc_macro_attribute]
        pub fn $i(args: TokenStream, item: TokenStream) -> TokenStream {
            $i::handle(parse_macro_input!(args), parse_macro_input!(item))
                .unwrap_or_else(Error::into_compile_error)
                .into()
        }
    )*};
}

export_attribute!(
    /// Converts the enum into ffi-safe `enum`s that could be safely sent through ffi Metadata, separated with
    /// `,`, passed as arguments of this attribute call would be transformed into raw attributes attached on
    /// the generated `ffi_enum`.
    ///
    /// The generated `ffi_enum` type implements `::ffi_enum::FfiEnum` that provides with essential type and constant
    /// informations of the generated details.
    ///
    /// The generated `ffi_enum` type by default implements `Copy`, `Clone`, `PartialEq`, `Eq`, based on which
    /// essential implementations could be derived.
    ///
    /// ## For Derive Macros
    ///
    /// There are 3 positions to place derive macros:
    ///
    /// 1. **(Preferred)** `#[derive(...)]` attached above `ffi_enum` would **see the original `enum` definition**
    /// and **`impl`s for the generated `ffi_enum` type**. Note that helper attributes attached below this
    /// `ffi_enum` attribute would also be seen by such derive macros.
    ///
    /// 2. `#[ffi_enum(derive(...))]` passed within this attribute would **both** see the definition of and takes
    /// all effects on **the generated `ffi_enum` type**. All helper attributes passed within this attribute
    /// would **only** be been by the derive macros passed in this way.
    ///
    /// 3. `#[derive(...)]` attached below `ffi_enum` would **both** see the definition of and takes
    /// all effects on the **the original `enum` definition**.
    ///
    /// Derive macros that works on regular `enum`s may simply works fine at Position `1` described above,
    /// unless the expanded code `match`es the value of the generated `ffi_enum` type **without a
    /// `_` wildcard pattern** that covers other unknown values that would potentially come from ffi.
    ///
    /// As for the derive macros that does not care the variant details, or normally logically works fine for a
    /// struct that contaions a single field,  attach the derive macros along with
    /// their helper attributes at Position `2`. Derive macros passed in this way would see a `ffi_enum_origin`
    /// helper attribute with the original enum definition passed as the arguments. If you are authoring a derive
    /// macro that may need to see both the original and generated definitions, this way is preferred.
    ///
    /// As for the derive macros that generate `match`ing code without wildcards, or the implemention of which is
    /// logically insufficient for the generated `ffi_enum` type that covers all binary patterns. The only
    /// way of automatic deriving is to `derive` at Position `3` described above, which implements for the original
    /// `enum` definition, and then delegate the implementations from the original `enum` definition which could
    /// accessed as `<TypeName as ffi_enum::FfiEnum>::Enum`.
    ///
    /// This attribute macro would automatically delegate implementations for the generated `ffi_enum` type
    /// for supported traits. The currently supported derives are:
    ///
    /// + `core::fmt::Debug`
    /// + `core::fmt::Display` (The derive macro for the original `enum` is not hereby specified, user may use any
    /// one that implements it for the original type)
    /// + `core::hash::Hash`
    /// + `core::str::FromStr` (The derive macro for the original `enum` is not hereby specified, user may use any
    /// one that implements it for the original type)
    /// + `thiserror::Error` implementing `core::error::Error` and `core::fmt::Display`
    /// + `serde::Serialize`
    /// + `serde::Deserialize`
    ///
    /// For more automatically delegatings, contributions are welcomed.
    ffi_enum
);

#[doc(hidden)]
#[proc_macro_derive(NoDerive, attributes(ffi_enum_origin))]
pub fn no_derive(_: TokenStream) -> TokenStream {
    TokenStream::new()
}

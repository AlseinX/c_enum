#![cfg_attr(not(test), no_std)]

use thiserror::Error;

#[doc(hidden)]
#[path = "private.rs"]
pub mod __private;
#[doc(inline)]
pub use macros::ffi_enum;

pub mod prelude {
    pub use crate::{ffi_enum, FfiEnum as _, FfiEnumExt as _};
}

/// types generated from `ffi_enum` implement this trait which provides essential type and constant information.
pub trait FfiEnum: Copy + Eq {
    /// The rusty `enum` type of the `ffi_enum` type
    type Enum: TryFrom<Self, Error = Error> + Into<Self>;

    /// The representation type of the `ffi_enum` type
    type Repr: Copy + From<Self> + Into<Self>;

    /// A sample of an unknown values
    ///
    /// Note that a `ffi_enum` accepts any value of the representation type, so `some_value != FfiEnum::UNKNOWN` **does not**
    /// indicate that `some_value` is known
    const UNKNOWN: Self;
}

/// Convenient operations on `FfiEnum`
pub trait FfiEnumExt: FfiEnum {
    /// Indicates whether the value is known
    fn is_known(self) -> bool;

    /// Gets the integer representation of the value
    fn repr(self) -> Self::Repr;

    /// Gets the rust enum of the value
    fn into_enum(self) -> Self::Enum;
}

impl<T: FfiEnum> FfiEnumExt for T {
    #[inline(always)]
    fn is_known(self) -> bool {
        Self::Enum::try_from(self).is_ok()
    }

    #[inline(always)]
    fn repr(self) -> Self::Repr {
        self.into()
    }

    #[inline(always)]
    fn into_enum(self) -> Self::Enum {
        self.try_into().unwrap()
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Error {
    #[error("value of ffi_enum type is an unknown variant")]
    UnknownVariant,
}

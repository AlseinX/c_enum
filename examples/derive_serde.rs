use ffi_enum::prelude::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[ffi_enum]
#[derive(Debug, Serialize, Deserialize, Error, Hash)]
#[serde(rename_all = "snake_case")]
#[repr(i16)]
pub enum Animal {
    #[error("unknown animal")]
    Cat,
    #[error("unknown animal")]
    Dog,
}

fn main() {
    let json = serde_json::to_string(&Animal::Cat).unwrap();
    assert_eq!(json, "\"cat\"");
    let value: Animal = serde_json::from_str(&json).unwrap();
    assert_eq!(value, Animal::Cat);
    let json = serde_json::to_string(&Animal::from(100i16)).unwrap();
    assert_eq!(json, "\"<Unknown>\"");
    let value: Animal = serde_json::from_str(&json).unwrap();
    assert_eq!(value, Animal::UNKNOWN);
    assert_eq!(usize::from(value), 2);
    assert_eq!(u8::try_from(value).unwrap(), 2);
}

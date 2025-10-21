use ffi_enum::prelude::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[ffi_enum]
#[derive(Debug, Serialize, Deserialize, Error, Hash)]
#[serde(rename_all = "snake_case")]
#[repr(i16)]
pub enum Animal {
    #[error("unknown animal")]
    #[serde(alias = "kitty")]
    Cat,
    #[error("unknown animal")]
    Dog,
}

fn main() {
    let json = serde_json::to_string(&Animal::Cat).unwrap();
    assert_eq!(json, "\"cat\"");
    let value: Animal = serde_json::from_str(&json).unwrap();
    assert_eq!(value, Animal::Cat);
    assert_eq!(value, serde_json::from_str("\"kitty\"").unwrap());
    assert!(serde_json::from_str::<Animal>("\"Rat\"").is_err());
    let value = Animal::from(2i16);
    let json = serde_json::to_string(&value).unwrap();
    assert_eq!(json, "\"<Unknown>\"");
    assert_eq!(usize::from(value), 2);
    assert_eq!(u8::try_from(value).unwrap(), 2);
}

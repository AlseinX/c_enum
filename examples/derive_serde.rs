use ffi_enum::{prelude::*, FfiEnum};
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

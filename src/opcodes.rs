use std::collections::HashMap;

use once_cell::unsync::Lazy;
use serde::{Deserialize, Serialize};

use crate::utils::Cycles;

pub(crate) const OPCODE_METADATA: Lazy<Opcodes<'static>> = Lazy::new(|| {
    let opcodes_file = include_str!("../opcodes.json");
    serde_json::from_str(opcodes_file).unwrap()
});

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Opcodes<'a> {
    #[serde(borrow)]
    pub unprefixed: HashMap<&'a str, Opcode<'a>>,

    #[serde(borrow)]
    pub cbprefixed: HashMap<&'a str, Opcode<'a>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Opcode<'a> {
    pub mnemonic: &'a str,
    pub bytes: u32,
    pub cycles: Vec<Cycles>,
    pub operands: Vec<Operand<'a>>,
    pub immediate: bool,
    pub flags: HashMap<&'a str, &'a str>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Operand<'a> {
    pub name: &'a str,
    pub immediate: bool,
    pub bytes: Option<u32>,
}

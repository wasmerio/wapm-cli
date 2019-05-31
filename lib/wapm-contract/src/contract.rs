//! The definition of a WAPM contract

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Contract {
    /// Things that the module can import
    pub imports: HashMap<String, Import>,
    /// Things that the module must export
    pub exports: HashMap<String, Export>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Import {
    Func {
        namespace: String,
        name: String,
        params: Vec<WasmType>,
        result: Vec<WasmType>,
    },
    Global {
        name: String,
        var_type: WasmType,
    },
}

// TODO: figure out this separator... '/' is a valid character in names so we can have collisions
impl Import {
    /// Get the key used to look this import up in the Contract's import hashmap
    pub fn get_key(&self) -> String {
        match self {
            Import::Func {
                namespace, name, ..
            } => format!("{}/{}", &namespace, &name),
            Import::Global { name, .. } => name.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Export {
    Func {
        name: String,
        params: Vec<WasmType>,
        result: Vec<WasmType>,
    },
    Global {
        name: String,
        var_type: WasmType,
    },
}

impl Export {
    /// Get the key used to look this export up in the Contract's export hashmap
    pub fn get_key(&self) -> String {
        match self {
            Export::Func { name, .. } => name.clone(),
            Export::Global { name, .. } => name.clone(),
        }
    }
}

/// Primitive wasm type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WasmType {
    I32,
    I64,
    F32,
    F64,
}

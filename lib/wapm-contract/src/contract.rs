//! The definition of a WAPM contract

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Contract {
    /// Things that the module can import
    pub imports: Vec<Import>,
    /// Things that the module must export
    pub exports: Vec<Export>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Import {
    Fn {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Export {
    Fn {
        name: String,
        params: Vec<WasmType>,
        result: Vec<WasmType>,
    },
    Global {
        name: String,
        var_type: WasmType,
    },
}

/// Primitive wasm type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WasmType {
    I32,
    I64,
    F32,
    F64,
}

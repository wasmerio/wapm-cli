use std::fmt;

/// The ABI is a hint to WebAssembly runtimes about what additional imports to insert.
/// It currently is only used for validation (in the validation subcommand).  The default value is `None`.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
pub enum Abi {
    #[serde(rename = "emscripten")]
    Emscripten,
    // TODO: figure out if this makes sense
    #[serde(rename = "")]
    None,
    #[serde(rename = "wasi")]
    Wasi,
}

impl fmt::Display for Abi {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Abi::Emscripten => "emscripten",
                Abi::Wasi => "wasi",
                Abi::None => "undefined ABI",
            }
        )
    }
}

impl Default for Abi {
    fn default() -> Self {
        Abi::None
    }
}

use std::fmt;

// TODO: update docstring
/// The ABI is a hint to WebAssembly runtimes about what additional imports to insert. For the time
/// being, this is a placeholder and does nothing. The default value is `None`.
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

/// The ABI is a hint to WebAssembly runtimes about what additional imports to insert. For the time
/// being, this is a placeholder and does nothing. The default value is `None`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum Abi {
    Emscripten,
    None,
    Wasi,
}

impl Default for Abi {
    fn default() -> Self {
        Abi::None
    }
}

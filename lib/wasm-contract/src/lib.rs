//! Definition and parsing of wasm contracts
//!
//! wasm contracts ensure wasm modules conform to a specific shape
//! they do this by asserting on the imports and exports of the module.

pub mod contract;
pub mod parser;

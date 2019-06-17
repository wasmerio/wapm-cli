//! Validate a wasm module given a contract.
//!
//! This checks that all imports are specified in the contract and that their types
//! are correct, as well as that all exports that the contract expects are exported
//! by the module and that their types are correct.

use crate::{Contract, Export, Import, WasmType};
use std::collections::HashMap;
use wasmparser::{ExternalKind, FuncType, GlobalType, ImportSectionEntryType};

pub fn validate_wasm_and_report_errors(
    wasm: &[u8],
    contract: &Contract,
) -> Result<(), WasmValidationError> {
    use wasmparser::WasmDecoder;

    let mut errors: Vec<String> = vec![];
    let mut import_fns: HashMap<(String, String), u32> = HashMap::new();
    let mut export_fns: HashMap<String, u32> = HashMap::new();
    let mut export_globals: HashMap<String, u32> = HashMap::new();
    let mut type_defs: Vec<FuncType> = vec![];
    let mut global_types: Vec<GlobalType> = vec![];

    let mut parser = wasmparser::ValidatingParser::new(wasm, None);
    loop {
        let state = parser.read();
        match state {
            wasmparser::ParserState::EndWasm => break,
            wasmparser::ParserState::Error(e) => {
                return Err(WasmValidationError::InvalidWasm {
                    error: format!("{}", e),
                }
                .into());
            }
            wasmparser::ParserState::ImportSectionEntry {
                module,
                field,
                ref ty,
            } => match ty {
                ImportSectionEntryType::Function(idx) => {
                    import_fns.insert(Import::format_key(module, field), *idx);
                }
                ImportSectionEntryType::Global(GlobalType { content_type, .. }) => {
                    let global_type =
                        wasmparser_type_into_wasm_type(*content_type).map_err(|err| {
                            WasmValidationError::UnsupportedType {
                                error: format!(
                                    "Invalid type found in import \"{}\" \"{}\": {}",
                                    module, field, err
                                ),
                            }
                        })?;
                    if let Some(val) = contract.imports.get(&Import::format_key(module, field)) {
                        if let Import::Global { var_type, .. } = val {
                            if *var_type != global_type {
                                errors.push(format!(
                                    "Invalid type on Global \"{}\". Expected {} found {}",
                                    field, var_type, global_type
                                ));
                            }
                        } else {
                            errors.push(format!(
                                "Invalid import type. Expected Global, found {:?}",
                                val
                            ));
                        }
                    } else {
                        errors.push(format!(
                            "Global import \"{}\" not found in the specified contract",
                            field
                        ));
                    }
                }
                _ => (),
            },
            wasmparser::ParserState::ExportSectionEntry {
                field,
                index,
                ref kind,
            } => match kind {
                ExternalKind::Function => {
                    export_fns.insert(Export::format_key(field), *index);
                }
                ExternalKind::Global => {
                    export_globals.insert(Export::format_key(field), *index);
                }
                _ => (),
            },
            wasmparser::ParserState::BeginGlobalSectionEntry(gt) => {
                global_types.push(gt.clone());
            }
            wasmparser::ParserState::TypeSectionEntry(ft) => {
                type_defs.push(ft.clone());
            }
            _ => {}
        }
    }

    validate_imports(&import_fns, &type_defs, contract, &mut errors);
    validate_export_fns(&export_fns, &type_defs, contract, &mut errors);
    validate_export_globals(&export_globals, &global_types, contract, &mut errors);

    if errors.is_empty() {
        Ok(())
    } else {
        Err(WasmValidationError::ContractViolated { errors: errors })
    }
}

/// Validates the import functions, checking the name and type against the given
/// `Contract`
fn validate_imports(
    import_fns: &HashMap<(String, String), u32>,
    type_defs: &Vec<FuncType>,
    contract: &Contract,
    errors: &mut Vec<String>,
) {
    for (key, val) in import_fns.iter() {
        if let Some(contract_def) = contract.imports.get(key) {
            let type_sig = if let Some(v) = type_defs.get(*val as usize) {
                v
            } else {
                errors.push(format!("Invalid type reference {}", val));
                continue;
            };
            if let Import::Func { params, result, .. } = contract_def {
                debug_assert!(type_sig.form == wasmparser::Type::Func);
                for (i, param) in type_sig
                    .params
                    .iter()
                    .cloned()
                    .map(wasmparser_type_into_wasm_type)
                    .enumerate()
                {
                    match param {
                        Ok(t) => {
                            if params.get(i).is_none() {
                                errors.push(format!("Found {} args but the contract only expects {} for imported function \"{}\" \"{}\"", i, params.len(), &key.0, &key.1));
                                continue;
                            }
                            if t != params[i] {
                                errors.push(format!(
                                    "Type mismatch in params in func \"{}\" \"{}\"",
                                    &key.0, &key.1
                                ));
                            }
                        }
                        Err(e) => errors.push(format!(
                            "Invalid type in func \"{}\" \"{}\": {}",
                            &key.0, &key.1, e
                        )),
                    }
                }
                for (i, ret) in type_sig
                    .returns
                    .iter()
                    .cloned()
                    .map(wasmparser_type_into_wasm_type)
                    .enumerate()
                {
                    match ret {
                        Ok(t) => {
                            if result.get(i).is_none() {
                                errors.push(format!("Found {} returns but the contract only expects {} for imported function \"{}\" \"{}\"", i, params.len(), &key.0, &key.1));
                                continue;
                            }

                            if t != result[i] {
                                errors.push(format!(
                                    "Type mismatch in returns in func \"{}\" \"{}\"",
                                    &key.0, &key.1
                                ));
                            }
                        }
                        Err(e) => errors.push(format!(
                            "Invalid type in func \"{}\" \"{}\": {}",
                            &key.0, &key.1, e
                        )),
                    }
                }
            }
        } else {
            // we didn't find the import at all in the contract
            // TODO: improve error messages by including type information
            errors.push(format!("Missing import \"{}\" \"{}\"", key.0, key.1));
        }
    }
}

/// Validates the export functions, checking the name and type against the given
/// `Contract`
fn validate_export_fns(
    export_fns: &HashMap<String, u32>,
    type_defs: &Vec<FuncType>,
    contract: &Contract,
    errors: &mut Vec<String>,
) {
    for (key, val) in export_fns.iter() {
        if let Some(contract_def) = contract.exports.get(key) {
            let type_sig = if let Some(v) = type_defs.get(*val as usize) {
                v
            } else {
                errors.push(format!("Invalid type reference {}", val));
                continue;
            };
            if let Export::Func { params, result, .. } = contract_def {
                debug_assert!(type_sig.form == wasmparser::Type::Func);
                for (i, param) in type_sig
                    .params
                    .iter()
                    .cloned()
                    .map(wasmparser_type_into_wasm_type)
                    .enumerate()
                {
                    match param {
                        Ok(t) => {
                            if params.get(i).is_none() {
                                errors.push(format!("Found {} args but the contract only expects {} for exported function \"{}\"", i, params.len(), &key));
                                continue;
                            }
                            if t != params[i] {
                                errors
                                    .push(format!("Type mismatch in params in func \"{}\"", &key));
                            }
                        }
                        Err(e) => errors.push(format!("Invalid type in func \"{}\": {}", &key, e)),
                    }
                }
                for (i, ret) in type_sig
                    .returns
                    .iter()
                    .cloned()
                    .map(wasmparser_type_into_wasm_type)
                    .enumerate()
                {
                    match ret {
                        Ok(t) => {
                            if result.get(i).is_none() {
                                errors.push(format!("Found {} returns but the contract only expects {} for exported function \"{}\"", i, params.len(), &key));
                                continue;
                            }

                            if t != result[i] {
                                errors
                                    .push(format!("Type mismatch in returns in func \"{}\"", &key));
                            }
                        }
                        Err(e) => errors.push(format!("Invalid type in func \"{}\": {}", &key, e)),
                    }
                }
            }
        }
    }
}

/// Validates the export globals, checking the name and type against the given
/// `Contract`
fn validate_export_globals(
    export_globals: &HashMap<String, u32>,
    global_types: &Vec<GlobalType>,
    contract: &Contract,
    errors: &mut Vec<String>,
) {
    for (key, val) in export_globals.iter() {
        if let Some(contract_def) = contract.exports.get(key) {
            if let Export::Global { var_type, .. } = contract_def {
                if global_types.get(*val as usize).is_none() {
                    errors.push(format!(
                        "Invalid wasm, expected {} global types, found {}",
                        val,
                        global_types.len()
                    ));
                }
                match wasmparser_type_into_wasm_type(global_types[*val as usize].content_type) {
                    Ok(t) => {
                        if *var_type != t {
                            errors.push(format!(
                                "Type mismatch in global export {}: expected {} found {}",
                                &key, var_type, t
                            ));
                        }
                    }
                    Err(e) => errors.push(format!("In global export {}: {}", &key, e)),
                }
            }
        }
    }
}

/// Converts Wasmparser's type enum into wasm-contract's type enum
/// wasmparser's enum contains things which are invalid in many situations
///
/// Additionally wasmerparser containers more advanced types like references that
/// wasm-contract does not yet support
fn wasmparser_type_into_wasm_type(ty: wasmparser::Type) -> Result<WasmType, String> {
    use wasmparser::Type;
    Ok(match ty {
        Type::I32 => WasmType::I32,
        Type::I64 => WasmType::I64,
        Type::F32 => WasmType::F32,
        Type::F64 => WasmType::F64,
        e => {
            return Err(format!("Invalid type found: {:?}", e));
        }
    })
}

#[cfg(test)]
mod validation_tests {
    use super::*;
    use crate::parser;

    #[test]
    fn global_imports() {
        const WAT: &str = r#"(module
(type $t0 (func (param i32 i64)))
(global $length (import "env" "length") i32)
(import "env" "do_panic" (func $do_panic (type $t0)))
)"#;
        let wasm = wabt::wat2wasm(WAT).unwrap();

        let contract_src = r#"
(assert_import (func "env" "do_panic" (param i32 i64)))
(assert_import (global "env" "length" (type i32)))"#;
        let contract = parser::parse_contract(contract_src).unwrap();

        let result = validate_wasm_and_report_errors(&wasm[..], &contract);

        assert!(result.is_ok());

        // Now set the global import type to mismatch the wasm
        let contract_src = r#"
(assert_import (func "env" "do_panic" (param i32 i64)))
(assert_import (global "env" "length" (type i64)))"#;
        let contract = parser::parse_contract(contract_src).unwrap();

        let result = validate_wasm_and_report_errors(&wasm[..], &contract);

        assert!(
            result.is_err(),
            "global import type mismatch causes an error"
        );

        // Now set the function import type to mismatch the wasm
        let contract_src = r#"
(assert_import (func "env" "do_panic" (param i64)))
(assert_import (global "env" "length" (type i32)))"#;
        let contract = parser::parse_contract(contract_src).unwrap();

        let result = validate_wasm_and_report_errors(&wasm[..], &contract);

        assert!(
            result.is_err(),
            "function import type mismatch causes an error"
        );

        // Now try with a module that has an import that the contract doesn't have
        let contract_src = r#"
(assert_import (func "env" "do_panic" (param i64)))
(assert_import (global "env" "length_plus_plus" (type i32)))"#;
        let contract = parser::parse_contract(contract_src).unwrap();

        let result = validate_wasm_and_report_errors(&wasm[..], &contract);

        assert!(
            result.is_err(),
            "all imports must be covered by the contract"
        );
    }

    #[test]
    fn global_exports() {
        const WAT: &str = r#"(module
(func (export "as-set_local-first") (param i32) (result i32)
  (nop) (i32.const 2) (set_local 0) (get_local 0))
(global (export "num_tries") i64 (i64.const 0))
)"#;
        let wasm = wabt::wat2wasm(WAT).unwrap();

        let contract_src = r#"
(assert_export (func "as-set_local-first" (param i32) (result i32)))
(assert_export (global "num_tries" (type i64)))"#;
        let contract = parser::parse_contract(contract_src).unwrap();

        let result = validate_wasm_and_report_errors(&wasm[..], &contract);

        assert!(result.is_ok());

        // Now set the global export type to mismatch the wasm
        let contract_src = r#"
(assert_export (func "as-set_local-first" (param i32) (result i32)))
(assert_export (global "num_tries" (type f32)))"#;
        let contract = parser::parse_contract(contract_src).unwrap();

        let result = validate_wasm_and_report_errors(&wasm[..], &contract);

        assert!(
            result.is_err(),
            "global export type mismatch causes an error"
        );

        // Now set the function export type to mismatch the wasm
        let contract_src = r#"
(assert_export (func "as-set_local-first" (param i64) (result i64)))
(assert_export (global "num_tries" (type i64)))"#;
        let contract = parser::parse_contract(contract_src).unwrap();

        let result = validate_wasm_and_report_errors(&wasm[..], &contract);

        assert!(
            result.is_err(),
            "function export type mismatch causes an error"
        );

        // Now try a contract that requires an export that the module doesn't have
        let contract_src = r#"
(assert_export (func "as-set_local-first" (param i64) (result i64)))
(assert_export (global "numb_trees" (type i64)))"#;
        let contract = parser::parse_contract(contract_src).unwrap();

        let result = validate_wasm_and_report_errors(&wasm[..], &contract);

        assert!(result.is_err(), "missing a required export is an error");
    }
}

#[derive(Debug)]
pub enum WasmValidationError {
    InvalidWasm { error: String },
    ContractViolated { errors: Vec<String> },
    UnsupportedType { error: String },
}

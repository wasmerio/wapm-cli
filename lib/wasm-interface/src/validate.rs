//! Validate a wasm module given a interface.
//!
//! This checks that all imports are specified in the interface and that their types
//! are correct, as well as that all exports that the interface expects are exported
//! by the module and that their types are correct.

use crate::{Export, Import, Interface, WasmType};
use std::collections::HashMap;
use wasmparser::{ExternalKind, FuncType, GlobalType, ImportSectionEntryType};

pub fn validate_wasm_and_report_errors(
    wasm: &[u8],
    interface: &Interface,
) -> Result<(), WasmValidationError> {
    use wasmparser::WasmDecoder;

    let mut errors: Vec<String> = vec![];
    let mut import_fns: HashMap<(String, String), u32> = HashMap::new();
    let mut export_fns: HashMap<String, u32> = HashMap::new();
    let mut export_globals: HashMap<String, u32> = HashMap::new();
    let mut type_defs: Vec<FuncType> = vec![];
    let mut global_types: Vec<GlobalType> = vec![];
    let mut fn_sigs: Vec<u32> = vec![];

    let mut parser = wasmparser::ValidatingParser::new(
        wasm,
        Some(wasmparser::ValidatingParserConfig {
            operator_config: wasmparser::OperatorValidatorConfig {
                enable_threads: true,
                enable_reference_types: true,
                enable_simd: true,
                enable_bulk_memory: true,
                enable_multi_value: true,
            },
        }),
    );
    loop {
        let state = parser.read();
        match state {
            wasmparser::ParserState::EndWasm => break,
            wasmparser::ParserState::Error(e) => {
                return Err(WasmValidationError::InvalidWasm {
                    error: format!("{}", e),
                });
            }
            wasmparser::ParserState::ImportSectionEntry {
                module,
                field,
                ref ty,
            } => match ty {
                ImportSectionEntryType::Function(idx) => {
                    import_fns.insert(Import::format_key(module, field), *idx);
                    fn_sigs.push(*idx);
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
                    if let Some(val) = interface.imports.get(&Import::format_key(module, field)) {
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
                            "Global import \"{}\" not found in the specified interface",
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
                global_types.push(*gt);
            }
            wasmparser::ParserState::TypeSectionEntry(ft) => {
                type_defs.push(ft.clone());
            }
            wasmparser::ParserState::FunctionSectionEntry(n) => {
                fn_sigs.push(*n);
            }
            _ => {}
        }
    }

    validate_imports(&import_fns, &type_defs, interface, &mut errors);
    validate_export_fns(&export_fns, &type_defs, &fn_sigs, interface, &mut errors);
    validate_export_globals(&export_globals, &global_types, interface, &mut errors);

    if errors.is_empty() {
        Ok(())
    } else {
        Err(WasmValidationError::InterfaceViolated { errors })
    }
}

/// Validates the import functions, checking the name and type against the given
/// `Interface`
fn validate_imports(
    import_fns: &HashMap<(String, String), u32>,
    type_defs: &[FuncType],
    interface: &Interface,
    errors: &mut Vec<String>,
) {
    for (key, val) in import_fns.iter() {
        if let Some(interface_def) = interface.imports.get(key) {
            let type_sig = if let Some(v) = type_defs.get(*val as usize) {
                v
            } else {
                errors.push(format!(
                    "Use of undeclared function reference \"{}\" in import function \"{}\" \"{}\"",
                    val, key.0, key.1
                ));
                continue;
            };
            if let Import::Func { params, result, .. } = interface_def {
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
                                errors.push(format!("Found {} args but the interface only expects {} for imported function \"{}\" \"{}\"", i, params.len(), &key.0, &key.1));
                                continue;
                            }
                            if t != params[i] {
                                errors.push(format!(
                                    "Type mismatch in params in imported func \"{}\" \"{}\": argument {}, expected {} found {}",
                                    &key.0, &key.1, i + 1, params[i], t
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
                                errors.push(format!("Found {} returns but the interface only expects {} for imported function \"{}\" \"{}\"", i, params.len(), &key.0, &key.1));
                                continue;
                            }

                            if t != result[i] {
                                errors.push(format!(
                                    "Type mismatch in returns in func \"{}\" \"{}\", return {}, expected {} found {}",
                                    &key.0, &key.1, i + 1, params[i], t
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
            // we didn't find the import at all in the interface
            // TODO: improve error messages by including type information
            errors.push(format!("Missing import \"{}\" \"{}\"", key.0, key.1));
        }
    }
}

/// Validates the export functions, checking the name and type against the given
/// `Interface`
fn validate_export_fns(
    export_fns: &HashMap<String, u32>,
    type_defs: &[FuncType],
    fn_sigs: &Vec<u32>,
    interface: &Interface,
    errors: &mut Vec<String>,
) {
    'export_loop: for (key, val) in export_fns.iter() {
        if let Some(interface_def) = interface.exports.get(key) {
            let type_sig = if let Some(type_idx) = fn_sigs.get(*val as usize) {
                if let Some(v) = type_defs.get(*type_idx as usize) {
                    v
                } else {
                    errors.push(format!(
                        "Export \"{}\" refers to type \"{}\" but only {} types were found",
                        &key,
                        type_idx,
                        fn_sigs.len()
                    ));
                    continue;
                }
            } else {
                errors.push(format!(
                    "Use of undeclared function reference \"{}\" in export \"{}\"",
                    val, &key
                ));
                continue;
            };
            if let Export::Func { params, result, .. } = interface_def {
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
                                errors.push(format!("Found {} args but the interface only expects {} for exported function \"{}\"", type_sig.params.len(), params.len(), &key));
                                continue 'export_loop;
                            }
                            if t != params[i] {
                                errors.push(format!(
                                    "Type mismatch in params in exported func \"{}\": in argument {}, expected {} found {}",
                                    &key, i + 1, params[i], t
                                ));
                            }
                        }
                        Err(e) => errors
                            .push(format!("Invalid type in exported func \"{}\": {}", &key, e)),
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
                                errors.push(format!("Found {} returns but the interface only expects {} for exported function \"{}\"", i, params.len(), &key));
                                continue 'export_loop;
                            }

                            if t != result[i] {
                                errors.push(format!(
                                    "Type mismatch in returns in exported func \"{}\": in return {}, expected {} found {}",
                                    &key, i + 1, result[i], t
                                ));
                            }
                        }
                        Err(e) => errors
                            .push(format!("Invalid type in exported func \"{}\": {}", &key, e)),
                    }
                }
            }
        }
    }
}

/// Validates the export globals, checking the name and type against the given
/// `Interface`
fn validate_export_globals(
    export_globals: &HashMap<String, u32>,
    global_types: &Vec<GlobalType>,
    interface: &Interface,
    errors: &mut Vec<String>,
) {
    for (key, val) in export_globals.iter() {
        if let Some(Export::Global { var_type, .. }) = interface.exports.get(key) {
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

/// Converts Wasmparser's type enum into wasm-interface's type enum
/// wasmparser's enum contains things which are invalid in many situations
///
/// Additionally wasmerparser containers more advanced types like references that
/// wasm-interface does not yet support
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
        let wasm = wat::parse_str(WAT).unwrap();

        let interface_src = r#"
(interface
(func (import "env" "do_panic") (param i32 i64))
(global (import "env" "length") (type i32)))"#;
        let interface = parser::parse_interface(interface_src).unwrap();

        let result = validate_wasm_and_report_errors(&wasm[..], &interface);

        assert!(result.is_ok());

        // Now set the global import type to mismatch the wasm
        let interface_src = r#"
(interface
(func (import "env" "do_panic") (param i32 i64))
(global (import "env" "length") (type i64)))"#;
        let interface = parser::parse_interface(interface_src).unwrap();

        let result = validate_wasm_and_report_errors(&wasm[..], &interface);

        assert!(
            result.is_err(),
            "global import type mismatch causes an error"
        );

        // Now set the function import type to mismatch the wasm
        let interface_src = r#"
(interface
(func (import "env" "do_panic") (param i64))
(global (import "env" "length") (type i32)))"#;
        let interface = parser::parse_interface(interface_src).unwrap();

        let result = validate_wasm_and_report_errors(&wasm[..], &interface);

        assert!(
            result.is_err(),
            "function import type mismatch causes an error"
        );

        // Now try with a module that has an import that the interface doesn't have
        let interface_src = r#"
(interface
(func (import "env" "do_panic") (param i64))
(global (import "env" "length_plus_plus") (type i32)))"#;
        let interface = parser::parse_interface(interface_src).unwrap();

        let result = validate_wasm_and_report_errors(&wasm[..], &interface);

        assert!(
            result.is_err(),
            "all imports must be covered by the interface"
        );
    }

    #[test]
    fn global_exports() {
        const WAT: &str = r#"(module
(func (export "as-set_local-first") (param i32) (result i32)
  (nop) (i32.const 2) (set_local 0) (get_local 0))
(global (export "num_tries") i64 (i64.const 0))
)"#;
        let wasm = wat::parse_str(WAT).unwrap();

        let interface_src = r#"
(interface
(func (export "as-set_local-first") (param i32) (result i32))
(global (export "num_tries") (type i64)))"#;
        let interface = parser::parse_interface(interface_src).unwrap();

        let result = validate_wasm_and_report_errors(&wasm[..], &interface);

        assert!(result.is_ok());

        // Now set the global export type to mismatch the wasm
        let interface_src = r#"
(interface
(func (export "as-set_local-first") (param i32) (result i32))
(global (export "num_tries") (type f32)))"#;
        let interface = parser::parse_interface(interface_src).unwrap();

        let result = validate_wasm_and_report_errors(&wasm[..], &interface);

        assert!(
            result.is_err(),
            "global export type mismatch causes an error"
        );

        // Now set the function export type to mismatch the wasm
        let interface_src = r#"
(interface
(func (export "as-set_local-first") (param i64) (result i64))
(global (export "num_tries") (type i64)))"#;
        let interface = parser::parse_interface(interface_src).unwrap();

        let result = validate_wasm_and_report_errors(&wasm[..], &interface);

        assert!(
            result.is_err(),
            "function export type mismatch causes an error"
        );

        // Now try a interface that requires an export that the module doesn't have
        let interface_src = r#"
(interface
(func (export "as-set_local-first") (param i64) (result i64))
(global (export "numb_trees") (type i64)))"#;
        let interface = parser::parse_interface(interface_src).unwrap();

        let result = validate_wasm_and_report_errors(&wasm[..], &interface);

        assert!(result.is_err(), "missing a required export is an error");
    }
}

#[derive(Debug)]
pub enum WasmValidationError {
    InvalidWasm { error: String },
    InterfaceViolated { errors: Vec<String> },
    UnsupportedType { error: String },
}

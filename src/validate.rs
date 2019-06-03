use crate::abi::Abi;
use crate::dataflow::manifest_packages::ManifestResult;
use std::collections::HashMap;
use std::{fs, io::Read, path::PathBuf};
use wasm_contract::{Contract, Export, Import, WasmType};
use wasmparser::{ExternalKind, FuncType, GlobalType, ImportSectionEntryType};

pub fn validate_directory(pkg_path: PathBuf) -> Result<(), failure::Error> {
    // validate as dir
    let manifest = match ManifestResult::find_in_directory(&pkg_path) {
        ManifestResult::NoManifest => return Ok(()),
        ManifestResult::ManifestError(e) => return Err(e.into()),
        ManifestResult::Manifest(manifest) => manifest,
    };
    if let Some(modules) = manifest.module {
        for module in modules.iter() {
            let source_path = if module.source.is_relative() {
                manifest.base_directory_path.join(&module.source)
            } else {
                module.source.clone()
            };
            let source_path_string = source_path.to_string_lossy().to_string();
            let mut wasm_file =
                fs::File::open(&source_path).map_err(|_| ValidationError::MissingFile {
                    file: source_path_string.clone(),
                })?;
            let mut wasm_buffer = Vec::new();
            wasm_file.read_to_end(&mut wasm_buffer).map_err(|err| {
                ValidationError::MiscCannotRead {
                    file: source_path_string.clone(),
                    error: format!("{}", err),
                }
            })?;
            let contract = Contract::default();
            let detected_abi =
                validate_wasm_and_report_errors(&wasm_buffer, &contract, source_path_string)?;

            if module.abi != Abi::None && module.abi != detected_abi {
                return Err(ValidationError::MismatchedABI {
                    module_name: module.name.clone(),
                    found_abi: detected_abi,
                    expected_abi: module.abi,
                }
                .into());
            }
        }
    }

    Ok(())
}

#[derive(Debug)]
pub struct WasmValidation {
    pub imports: Vec<Import>,
    pub exports: Vec<Export>,
}

pub fn validate_wasm_and_report_errors(
    wasm: &[u8],
    contract: &Contract,
    file_name: String,
) -> Result<Abi, failure::Error> {
    use wasmparser::WasmDecoder;

    let mut errors: Vec<String> = vec![];
    let mut import_fns: HashMap<String, u32> = HashMap::new();
    let mut export_fns: HashMap<String, u32> = HashMap::new();
    let mut export_globals: HashMap<String, u32> = HashMap::new();
    let mut type_defs: Vec<FuncType> = vec![];
    let mut global_types: Vec<GlobalType> = vec![];

    let mut parser = wasmparser::ValidatingParser::new(wasm, None);
    let mut abi = Abi::None;
    loop {
        let state = parser.read();
        match state {
            wasmparser::ParserState::EndWasm => break,
            wasmparser::ParserState::Error(e) => {
                return Err(ValidationError::InvalidWasm {
                    file: file_name,
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
                    import_fns.insert(Import::format_fn_key(module, field), *idx);
                }
                ImportSectionEntryType::Global(GlobalType {
                    content_type,
                    mutable,
                }) => {
                    let global_type =
                        wasmparser_type_into_wasm_type(*content_type).map_err(|err| {
                            format_err!(
                                "Invalid type found in import \"{}\" \"{}\": {}",
                                module,
                                field,
                                err
                            )
                        })?;
                    if let Some(val) = contract.imports.get(&Import::format_global_key(field)) {
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
                    export_fns.insert(Export::format_fn_key(field), *index);
                }
                ExternalKind::Global => {
                    export_globals.insert(Export::format_global_key(field), *index);
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
        Ok(abi)
    } else {
        Err(format_err!(
            "Error validating contract:{}",
            errors
                .into_iter()
                .fold(String::new(), |a, b| { a + "\n" + &b })
        ))
    }
}

/// Validates the import functions, checking the name and type against the given
/// `Contract`
fn validate_imports(
    import_fns: &HashMap<String, u32>,
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
                                errors.push(format!("Found {} args but the contract only expects {} for imported function \"{}\"", i, params.len(), &key));
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
                                errors.push(format!("Found {} returns but the contract only expects {} for imported function \"{}\"", i, params.len(), &key));
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

#[derive(Debug, Fail)]
pub enum ValidationError {
    #[fail(
        display = "WASM file \"{}\" detected as invalid because {}",
        file, error
    )]
    InvalidWasm { file: String, error: String },
    #[fail(display = "Could not find file {}", file)]
    MissingFile { file: String },
    #[fail(display = "Failed to read file {}; {}", file, error)]
    MiscCannotRead { file: String, error: String },
    #[fail(
        display = "Multiple ABIs detected in file {}; previously detected {} but found {}",
        file, first_abi, second_abi
    )]
    MultipleABIs {
        file: String,
        first_abi: Abi,
        second_abi: Abi,
    },
    #[fail(
        display = "Detected ABI ({}) does not match ABI specified in wapm.toml ({}) for module \"{}\"",
        found_abi, expected_abi, module_name
    )]
    MismatchedABI {
        module_name: String,
        found_abi: Abi,
        expected_abi: Abi,
    },
    #[fail(display = "Failed to unpack archive \"{}\"! {}", file, error)]
    CannotUnpackArchive { file: String, error: String },
}

#[cfg(test)]
mod validation_tests {
    use super::*;

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
(assert_import (global "length" (type i32)))"#;
        let contract = wasm_contract::parser::parse_contract(contract_src).unwrap();

        let result = validate_wasm_and_report_errors(
            &wasm[..],
            &contract,
            "global_imports_test".to_string(),
        );

        assert!(result.is_ok());

        // Now set the global import type to mismatch the wasm
        let contract_src = r#"
(assert_import (func "env" "do_panic" (param i32 i64)))
(assert_import (global "length" (type i64)))"#;
        let contract = wasm_contract::parser::parse_contract(contract_src).unwrap();

        let result = validate_wasm_and_report_errors(
            &wasm[..],
            &contract,
            "global_imports_test".to_string(),
        );

        assert!(
            result.is_err(),
            "global import type mismatch causes an error"
        );

        // Now set the function import type to mismatch the wasm
        let contract_src = r#"
(assert_import (func "env" "do_panic" (param i64)))
(assert_import (global "length" (type i32)))"#;
        let contract = wasm_contract::parser::parse_contract(contract_src).unwrap();

        let result = validate_wasm_and_report_errors(
            &wasm[..],
            &contract,
            "global_imports_test".to_string(),
        );

        assert!(
            result.is_err(),
            "function import type mismatch causes an error"
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
        let contract = wasm_contract::parser::parse_contract(contract_src).unwrap();

        let result = validate_wasm_and_report_errors(
            &wasm[..],
            &contract,
            "global_imports_test".to_string(),
        );

        assert!(result.is_ok());

        // Now set the global export type to mismatch the wasm
        let contract_src = r#"
(assert_export (func "as-set_local-first" (param i32) (result i32)))
(assert_export (global "num_tries" (type f32)))"#;
        let contract = wasm_contract::parser::parse_contract(contract_src).unwrap();

        let result = validate_wasm_and_report_errors(
            &wasm[..],
            &contract,
            "global_exports_test".to_string(),
        );

        assert!(
            result.is_err(),
            "global export type mismatch causes an error"
        );

        // Now set the function export type to mismatch the wasm
        let contract_src = r#"
(assert_export (func "as-set_local-first" (param i64) (result i64)))
(assert_export (global "num_tries" (type i64)))"#;
        let contract = wasm_contract::parser::parse_contract(contract_src).unwrap();

        let result = validate_wasm_and_report_errors(
            &wasm[..],
            &contract,
            "global_exports_test".to_string(),
        );

        assert!(
            result.is_err(),
            "function export type mismatch causes an error"
        );
    }
}

//! Parsers to get a wasm contract from text
//! The grammar of the text format is:
//! DECL_TYPE = assert_import | assert_export
//! IMPORT_DATA =
//! EXPORT_DATA =
//! (DECL_TYPE ())

use nom::{
    branch::*,
    bytes::complete::{escaped, is_not, tag},
    character::complete::{char, multispace0, multispace1, one_of},
    combinator::*,
    error::context,
    multi::{many0, many1},
    sequence::{delimited, preceded, tuple},
    IResult,
};

use crate::contract::*;

/// Some example input:
/// (assert_import (func "ns" "name" (param f64 i32) (result f64 i32)))
/// (assert_export (func "name" (param f64 i32) (result f64 i32)))
/// (assert_import (global "name" (type f64)))

pub fn parse_contract(mut input: &str) -> Result<Contract, String> {
    let mut import_found = true;
    let mut export_found = true;
    let mut contract = Contract::default();
    while import_found || export_found {
        if let Result::Ok((inp, out)) = preceded(space_comments, parse_imports)(input) {
            for entry in out.into_iter() {
                if let Some(dup) = contract.imports.insert(entry.get_key(), entry) {
                    return Err(format!("Duplicate import found {:?}", dup));
                }
            }
            input = inp;
            import_found = true;
        } else {
            import_found = false;
        }

        if let Result::Ok((inp, out)) = preceded(space_comments, parse_exports)(input) {
            for entry in out.into_iter() {
                if let Some(dup) = contract.exports.insert(entry.get_key(), entry) {
                    return Err(format!("Duplicate export found {:?}", dup));
                }
            }
            input = inp;
            export_found = true;
        } else {
            export_found = false;
        }
    }
    if !input.is_empty() {
        Err(format!("Could not parse remaining input: {}", input))
    } else {
        Ok(contract)
    }
}

fn parse_comment(input: &str) -> IResult<&str, ()> {
    map(
        preceded(multispace0, preceded(char(';'), many0(is_not("\n")))),
        |_| (),
    )(input)
}

/// Consumes spaces and comments
/// comments must terminate with a new line character
fn space_comments<'a>(mut input: &'a str) -> IResult<&'a str, ()> {
    let mut space_found = true;
    let mut comment_found = true;
    while space_found || comment_found {
        let space: IResult<&'a str, _> = multispace1(input);
        space_found = if let Result::Ok((inp, _)) = space {
            input = inp;
            true
        } else {
            false
        };
        comment_found = if let Result::Ok((inp, _)) = parse_comment(input) {
            input = inp;
            true
        } else {
            false
        };
    }
    Ok((input, ()))
}

fn parse_imports(input: &str) -> IResult<&str, Vec<Import>> {
    let parse_import_inner = context(
        "assert_import",
        preceded(
            tag("assert_import"),
            many1(preceded(space_comments, alt((func_import, global_import)))),
        ),
    );
    s_exp(parse_import_inner)(input)
}

fn parse_exports(input: &str) -> IResult<&str, Vec<Export>> {
    let parse_export_inner = context(
        "assert_export",
        preceded(
            tag("assert_export"),
            many1(preceded(space_comments, alt((func_export, global_export)))),
        ),
    );
    s_exp(parse_export_inner)(input)
}

/// A quoted identifier, must be valid UTF8
fn identifier(input: &str) -> IResult<&str, &str> {
    let name_inner = escaped(is_not("\"\\"), '\\', one_of("\"n\\"));
    context("identifier", delimited(char('"'), name_inner, char('"')))(input)
}

/// Parses a wasm primitive type
fn wasm_type(input: &str) -> IResult<&str, WasmType> {
    let i32_tag = map(tag("i32"), |_| WasmType::I32);
    let i64_tag = map(tag("i64"), |_| WasmType::I64);
    let f32_tag = map(tag("f32"), |_| WasmType::F32);
    let f64_tag = map(tag("f64"), |_| WasmType::F64);

    alt((i32_tag, i64_tag, f32_tag, f64_tag))(input)
}

/// Parses an S-expression
fn s_exp<'a, O1, F>(inner: F) -> impl Fn(&'a str) -> IResult<&'a str, O1>
where
    F: Fn(&'a str) -> IResult<&'a str, O1>,
{
    delimited(
        char('('),
        preceded(space_comments, inner),
        preceded(space_comments, char(')')),
    )
}

/// (global "name" (type f64))
fn global_import(input: &str) -> IResult<&str, Import> {
    let global_type_inner = preceded(tag("type"), preceded(space_comments, wasm_type));
    let type_s_exp = s_exp(global_type_inner);
    let global_import_inner = context(
        "global import inner",
        preceded(
            tag("global"),
            map(
                tuple((
                    preceded(space_comments, identifier),
                    preceded(space_comments, identifier),
                    preceded(space_comments, type_s_exp),
                )),
                |(ns, name, var_type)| Import::Global {
                    namespace: ns.to_string(),
                    name: name.to_string(),
                    var_type,
                },
            ),
        ),
    );
    s_exp(global_import_inner)(input)
}

/// (global "name" (type f64))
fn global_export(input: &str) -> IResult<&str, Export> {
    let global_type_inner = preceded(tag("type"), preceded(space_comments, wasm_type));
    let type_s_exp = s_exp(global_type_inner);
    let global_export_inner = context(
        "global export inner",
        preceded(
            tag("global"),
            map(
                tuple((
                    preceded(space_comments, identifier),
                    preceded(space_comments, type_s_exp),
                )),
                |(name, var_type)| Export::Global {
                    name: name.to_string(),
                    var_type,
                },
            ),
        ),
    );
    s_exp(global_export_inner)(input)
}

/// (func "ns" "name" (param f64 i32) (result f64 i32))
fn func_import(input: &str) -> IResult<&str, Import> {
    let param_list_inner = preceded(tag("param"), many0(preceded(space_comments, wasm_type)));
    let param_list = opt(s_exp(param_list_inner));
    let result_list_inner = preceded(tag("result"), many0(preceded(space_comments, wasm_type)));
    let result_list = opt(s_exp(result_list_inner));
    let func_import_inner = context(
        "func import inner",
        preceded(
            tag("func"),
            map(
                tuple((
                    preceded(space_comments, identifier),
                    preceded(space_comments, identifier),
                    preceded(space_comments, param_list),
                    preceded(space_comments, result_list),
                )),
                |(ns, name, pl, rl)| Import::Func {
                    namespace: ns.to_string(),
                    name: name.to_string(),
                    params: pl.unwrap_or_default(),
                    result: rl.unwrap_or_default(),
                },
            ),
        ),
    );
    s_exp(func_import_inner)(input)
}

/// (func "name" (param f64 i32) (result f64 i32))
fn func_export(input: &str) -> IResult<&str, Export> {
    let param_list_inner = preceded(tag("param"), many0(preceded(space_comments, wasm_type)));
    let param_list = opt(s_exp(param_list_inner));
    let result_list_inner = preceded(tag("result"), many0(preceded(space_comments, wasm_type)));
    let result_list = opt(s_exp(result_list_inner));
    let func_export_inner = context(
        "func export inner",
        preceded(
            tag("func"),
            map(
                tuple((
                    preceded(space_comments, identifier),
                    preceded(space_comments, param_list),
                    preceded(space_comments, result_list),
                )),
                |(name, pl, rl)| Export::Func {
                    name: name.to_string(),
                    params: pl.unwrap_or_default(),
                    result: rl.unwrap_or_default(),
                },
            ),
        ),
    );
    s_exp(func_export_inner)(input)
}

#[cfg(test)]
mod test {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn parse_wasm_type() {
        let i32_res = wasm_type("i32").unwrap();
        assert_eq!(i32_res, ("", WasmType::I32));
        let i64_res = wasm_type("i64").unwrap();
        assert_eq!(i64_res, ("", WasmType::I64));
        let f32_res = wasm_type("f32").unwrap();
        assert_eq!(f32_res, ("", WasmType::F32));
        let f64_res = wasm_type("f64").unwrap();
        assert_eq!(f64_res, ("", WasmType::F64));

        assert!(wasm_type("i128").is_err());
    }

    #[test]
    fn parse_identifier() {
        let inner_str = "柴は可愛すぎるだと思います";
        let input = format!("\"{}\"", &inner_str);
        let parse_res = identifier(&input).unwrap();
        assert_eq!(parse_res, ("", inner_str))
    }

    #[test]
    fn parse_global_import() {
        let parse_res = global_import("(global \"env\" \"length\" (type i32))").unwrap();
        assert_eq!(
            parse_res,
            (
                "",
                Import::Global {
                    namespace: "env".to_string(),
                    name: "length".to_string(),
                    var_type: WasmType::I32,
                }
            )
        );
    }

    #[test]
    fn parse_global_export() {
        let parse_res = global_export("(global \"length\" (type i32))").unwrap();
        assert_eq!(
            parse_res,
            (
                "",
                Export::Global {
                    name: "length".to_string(),
                    var_type: WasmType::I32,
                }
            )
        );
    }

    #[test]
    fn parse_func_import() {
        let parse_res =
            func_import("(func \"ns\" \"name\" (param f64 i32) (result f64 i32))").unwrap();
        assert_eq!(
            parse_res,
            (
                "",
                Import::Func {
                    namespace: "ns".to_string(),
                    name: "name".to_string(),
                    params: vec![WasmType::F64, WasmType::I32],
                    result: vec![WasmType::F64, WasmType::I32],
                }
            )
        );
    }

    #[test]
    fn parse_func_export() {
        let parse_res = func_export("(func \"name\" (param f64 i32) (result f64 i32))").unwrap();
        assert_eq!(
            parse_res,
            (
                "",
                Export::Func {
                    name: "name".to_string(),
                    params: vec![WasmType::F64, WasmType::I32],
                    result: vec![WasmType::F64, WasmType::I32],
                }
            )
        );
    }

    #[test]
    fn parse_imports_test() {
        let parse_res = parse_imports(
            "(assert_import (func \"ns\" \"name\" (param f64 i32) (result f64 i32)))",
        )
        .unwrap();
        assert_eq!(
            parse_res,
            (
                "",
                vec![Import::Func {
                    namespace: "ns".to_string(),
                    name: "name".to_string(),
                    params: vec![WasmType::F64, WasmType::I32],
                    result: vec![WasmType::F64, WasmType::I32],
                }]
            )
        );

        let parse_res = parse_imports(
            "(assert_import (func \"ns\" \"name\"  
                                               (param f64 i32) (result f64 i32))
    ( global \"env\" \"length\" ( type 
;; i32 is the best type
i32 )
)
                                          (func \"ns\" \"name2\" (param f32
                                                                      i64)
                               ;; The return value comes next
                                                                (
                                                                 result
                                                                 f64
                                                                 i32
                                                                 )
                                          ) 
)",
        )
        .unwrap();
        assert_eq!(
            parse_res,
            (
                "",
                vec![
                    Import::Func {
                        namespace: "ns".to_string(),
                        name: "name".to_string(),
                        params: vec![WasmType::F64, WasmType::I32],
                        result: vec![WasmType::F64, WasmType::I32],
                    },
                    Import::Global {
                        namespace: "env".to_string(),
                        name: "length".to_string(),
                        var_type: WasmType::I32,
                    },
                    Import::Func {
                        namespace: "ns".to_string(),
                        name: "name2".to_string(),
                        params: vec![WasmType::F32, WasmType::I64],
                        result: vec![WasmType::F64, WasmType::I32],
                    },
                ]
            )
        );
    }

    #[test]
    fn top_level_test() {
        let parse_res = parse_contract(
            " (assert_import (func \"ns\" \"name\" (param f64 i32) (result f64 i32)))
 (assert_export (func \"name2\" (param) (result i32)))
 (assert_import (global \"env\" \"length\" (type f64)))",
        )
        .unwrap();

        let imports = vec![
            Import::Func {
                namespace: "ns".to_string(),
                name: "name".to_string(),
                params: vec![WasmType::F64, WasmType::I32],
                result: vec![WasmType::F64, WasmType::I32],
            },
            Import::Global {
                namespace: "env".to_string(),
                name: "length".to_string(),
                var_type: WasmType::F64,
            },
        ];
        let exports = vec![Export::Func {
            name: "name2".to_string(),
            params: vec![],
            result: vec![WasmType::I32],
        }];
        let import_map = imports
            .into_iter()
            .map(|entry| (entry.get_key(), entry))
            .collect::<HashMap<(String, String), Import>>();
        let export_map = exports
            .into_iter()
            .map(|entry| (entry.get_key(), entry))
            .collect::<HashMap<String, Export>>();
        assert_eq!(
            parse_res,
            Contract {
                imports: import_map,
                exports: export_map,
            }
        );
    }

    #[test]
    fn duplicates_not_allowed() {
        let parse_res = parse_contract(
            " (assert_import (func \"ns\" \"name\" (param f64 i32) (result f64 i32)))
; test comment
  ;; hello
 (assert_import (func \"ns\" \"name\" (param) (result i32)))
 (assert_import (global \"length\" (type f64)))

",
        );

        assert!(parse_res.is_err());
    }

    #[test]
    fn test_comment_space_parsing() {
        let parse_res = space_comments(" ").unwrap();
        assert_eq!(parse_res, ("", ()));
        let parse_res = space_comments("").unwrap();
        assert_eq!(parse_res, ("", ()));
        let parse_res = space_comments("; hello\n").unwrap();
        assert_eq!(parse_res, ("", ()));
        let parse_res = space_comments("abc").unwrap();
        assert_eq!(parse_res, ("abc", ()));
    }

    #[test]
    fn test_param_elision() {
        let parse_res = parse_contract(
            " (assert_import (func \"ns\" \"name\" (result f64 i32)))
(assert_export (func \"name\"))",
        )
        .unwrap();

        let imports = vec![Import::Func {
            namespace: "ns".to_string(),
            name: "name".to_string(),
            params: vec![],
            result: vec![WasmType::F64, WasmType::I32],
        }];
        let exports = vec![Export::Func {
            name: "name".to_string(),
            params: vec![],
            result: vec![],
        }];
        let import_map = imports
            .into_iter()
            .map(|entry| (entry.get_key(), entry))
            .collect::<HashMap<(String, String), Import>>();
        let export_map = exports
            .into_iter()
            .map(|entry| (entry.get_key(), entry))
            .collect::<HashMap<String, Export>>();
        assert_eq!(
            parse_res,
            Contract {
                imports: import_map,
                exports: export_map,
            }
        );
    }

    #[test]
    fn typo_gets_caught() {
        let contract_src = r#"
(assert_import (func "env" "do_panic" (params i32 i64)))
(assert_import (global "length" (type i32)))"#;
        let result = parse_contract(contract_src);
        assert!(result.is_err());
    }
}

use nom::{
    branch::*,
    bytes::complete::{escaped, is_not, tag},
    character::complete::{char, multispace0, one_of},
    combinator::*,
    error::{context, ParseError},
    multi::{many0, many1},
    sequence::{delimited, preceded, tuple},
    IResult,
};

use crate::contract::*;

/// Some example input:
/// (assert-import (fn "ns" "name" (param f64 i32) (result f64 i32)))
/// (assert-export (fn "name" (param f64 i32) (result f64 i32)))
/// (assert-import (global "name" (type f64)))

pub fn parse_contract(mut input: &str) -> IResult<&str, Contract> {
    let mut import_found = true;
    let mut export_found = true;
    let mut contract = Contract::default();
    while import_found || export_found {
        if let Result::Ok((inp, mut out)) = preceded(multispace0, parse_imports)(input) {
            contract.imports.append(&mut out);
            input = inp;
            import_found = true;
        } else {
            import_found = false;
        }

        if let Result::Ok((inp, mut out)) = preceded(multispace0, parse_exports)(input) {
            contract.exports.append(&mut out);
            input = inp;
            export_found = true;
        } else {
            export_found = false;
        }
    }
    Ok((input, contract))
}

fn parse_imports(input: &str) -> IResult<&str, Vec<Import>> {
    let parse_import_inner = context(
        "assert-import",
        preceded(
            tag("assert-import"),
            many1(preceded(multispace0, alt((fn_import, global_import)))),
        ),
    );
    sexp(parse_import_inner)(input)
}

fn parse_exports(input: &str) -> IResult<&str, Vec<Export>> {
    let parse_export_inner = context(
        "assert-export",
        preceded(
            tag("assert-export"),
            many1(preceded(multispace0, alt((fn_export, global_export)))),
        ),
    );
    sexp(parse_export_inner)(input)
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
fn sexp<'a, O1, E: ParseError<&'a str>, F>(inner: F) -> impl Fn(&'a str) -> IResult<&'a str, O1, E>
where
    F: Fn(&'a str) -> IResult<&'a str, O1, E>,
{
    delimited(
        char('('),
        preceded(multispace0, inner),
        preceded(multispace0, char(')')),
    )
}

/// (global "name" (type f64))
fn global_import(input: &str) -> IResult<&str, Import> {
    let global_type_inner = preceded(tag("type"), preceded(multispace0, wasm_type));
    let type_sexp = sexp(global_type_inner);
    let global_import_inner = context(
        "global import inner",
        preceded(
            tag("global"),
            map(
                tuple((
                    preceded(multispace0, identifier),
                    preceded(multispace0, type_sexp),
                )),
                |(name, var_type)| Import::Global {
                    name: name.to_string(),
                    var_type,
                },
            ),
        ),
    );
    sexp(global_import_inner)(input)
}

/// (global "name" (type f64))
fn global_export(input: &str) -> IResult<&str, Export> {
    let global_type_inner = preceded(tag("type"), preceded(multispace0, wasm_type));
    let type_sexp = sexp(global_type_inner);
    let global_export_inner = context(
        "global export inner",
        preceded(
            tag("global"),
            map(
                tuple((
                    preceded(multispace0, identifier),
                    preceded(multispace0, type_sexp),
                )),
                |(name, var_type)| Export::Global {
                    name: name.to_string(),
                    var_type,
                },
            ),
        ),
    );
    sexp(global_export_inner)(input)
}

/// (fn "ns" "name" (param f64 i32) (result f64 i32))
fn fn_import(input: &str) -> IResult<&str, Import> {
    let param_list_inner = preceded(tag("param"), many0(preceded(multispace0, wasm_type)));
    let param_list = sexp(param_list_inner);
    let result_list_inner = preceded(tag("result"), many0(preceded(multispace0, wasm_type)));
    let result_list = sexp(result_list_inner);
    let fn_import_inner = context(
        "fn import inner",
        preceded(
            tag("fn"),
            map(
                tuple((
                    preceded(multispace0, identifier),
                    preceded(multispace0, identifier),
                    preceded(multispace0, param_list),
                    preceded(multispace0, result_list),
                )),
                |(ns, name, pl, rl)| Import::Fn {
                    namespace: ns.to_string(),
                    name: name.to_string(),
                    params: pl,
                    result: rl,
                },
            ),
        ),
    );
    sexp(fn_import_inner)(input)
}

/// (fn "name" (param f64 i32) (result f64 i32))
fn fn_export(input: &str) -> IResult<&str, Export> {
    let param_list_inner = preceded(tag("param"), many0(preceded(multispace0, wasm_type)));
    let param_list = sexp(param_list_inner);
    let result_list_inner = preceded(tag("result"), many0(preceded(multispace0, wasm_type)));
    let result_list = sexp(result_list_inner);
    let fn_export_inner = context(
        "fn export inner",
        preceded(
            tag("fn"),
            map(
                tuple((
                    preceded(multispace0, identifier),
                    preceded(multispace0, param_list),
                    preceded(multispace0, result_list),
                )),
                |(name, pl, rl)| Export::Fn {
                    name: name.to_string(),
                    params: pl,
                    result: rl,
                },
            ),
        ),
    );
    sexp(fn_export_inner)(input)
}

#[cfg(test)]
mod test {
    use super::*;

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
        let parse_res = global_import("(global \"length\" (type i32))").unwrap();
        assert_eq!(
            parse_res,
            (
                "",
                Import::Global {
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
    fn parse_fn_import() {
        let parse_res = fn_import("(fn \"ns\" \"name\" (param f64 i32) (result f64 i32))").unwrap();
        assert_eq!(
            parse_res,
            (
                "",
                Import::Fn {
                    namespace: "ns".to_string(),
                    name: "name".to_string(),
                    params: vec![WasmType::F64, WasmType::I32],
                    result: vec![WasmType::F64, WasmType::I32],
                }
            )
        );
    }

    #[test]
    fn parse_fn_export() {
        let parse_res = fn_export("(fn \"name\" (param f64 i32) (result f64 i32))").unwrap();
        assert_eq!(
            parse_res,
            (
                "",
                Export::Fn {
                    name: "name".to_string(),
                    params: vec![WasmType::F64, WasmType::I32],
                    result: vec![WasmType::F64, WasmType::I32],
                }
            )
        );
    }

    #[test]
    fn parse_imports_test() {
        let parse_res =
            parse_imports("(assert-import (fn \"ns\" \"name\" (param f64 i32) (result f64 i32)))")
                .unwrap();
        assert_eq!(
            parse_res,
            (
                "",
                vec![Import::Fn {
                    namespace: "ns".to_string(),
                    name: "name".to_string(),
                    params: vec![WasmType::F64, WasmType::I32],
                    result: vec![WasmType::F64, WasmType::I32],
                }]
            )
        );

        let parse_res = parse_imports(
            "(assert-import (fn \"ns\" \"name\"  
                                               (param f64 i32) (result f64 i32))
    ( global \"length\" ( type 
i32 )
)
                                          (fn \"ns\" \"name2\" (param f32
                                                                      i64)
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
                    Import::Fn {
                        namespace: "ns".to_string(),
                        name: "name".to_string(),
                        params: vec![WasmType::F64, WasmType::I32],
                        result: vec![WasmType::F64, WasmType::I32],
                    },
                    Import::Global {
                        name: "length".to_string(),
                        var_type: WasmType::I32,
                    },
                    Import::Fn {
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
            " (assert-import (fn \"ns\" \"name\" (param f64 i32) (result f64 i32)))
 (assert-export (fn \"name2\" (param) (result i32)))
 (assert-import (global \"length\" (type f64)))",
        )
        .unwrap();

        assert_eq!(
            parse_res,
            (
                "",
                Contract {
                    imports: vec![
                        Import::Fn {
                            namespace: "ns".to_string(),
                            name: "name".to_string(),
                            params: vec![WasmType::F64, WasmType::I32],
                            result: vec![WasmType::F64, WasmType::I32],
                        },
                        Import::Global {
                            name: "length".to_string(),
                            var_type: WasmType::F64,
                        }
                    ],
                    exports: vec![Export::Fn {
                        name: "name2".to_string(),
                        params: vec![],
                        result: vec![WasmType::I32]
                    }]
                }
            )
        );
    }
}

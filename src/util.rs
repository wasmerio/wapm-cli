use crate::graphql::execute_query;
use graphql_client::*;

pub static MAX_PACKAGE_NAME_LENGTH: usize = 50;

#[derive(Debug, Fail)]
pub enum PackageNameError {
    #[fail(
        display = "Package name, \"{}\", is too long, name must be {} characters or fewer",
        _0, _1
    )]
    NameTooLong(String, usize),
    #[fail(
        display = "Package name, \"{}\", contains invalid characters.  Please use alpha-numeric characters, '-', and '_'",
        _0
    )]
    InvalidCharacters(String),
}

/// Checks whether a given package name is acceptable or not
pub fn validate_package_name(package_name: &str) -> Result<(), PackageNameError> {
    if package_name.len() > MAX_PACKAGE_NAME_LENGTH {
        return Err(PackageNameError::NameTooLong(
            package_name.to_string(),
            MAX_PACKAGE_NAME_LENGTH,
        ));
    }

    let re = regex::Regex::new("^[-a-zA-Z0-9_]+").unwrap();

    if !re.is_match(package_name) {
        return Err(PackageNameError::InvalidCharacters(
            package_name.to_string(),
        ));
    }

    Ok(())
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/whoami.graphql",
    response_derives = "Debug"
)]
struct WhoAmIQuery;

pub fn get_username() -> Result<Option<String>, failure::Error> {
    let q = WhoAmIQuery::build_query(who_am_i_query::Variables {});
    let response: who_am_i_query::ResponseData = execute_query(&q)?;
    Ok(response.viewer.map(|viewer| viewer.username))
}

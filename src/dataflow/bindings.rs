use std::fmt::{self, Display, Formatter};

use graphql_client::GraphQLQuery;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Query(anyhow::Error),
    #[error("Package not found in the registry {registry:?}: {name}")]
    UnknownPackage { name: String, registry: String },
    #[error("{package_name} v{version} doesn't have any {language} bindings")]
    MissingBindings {
        package_name: String,
        version: String,
        language: Language,
    },
    #[error("The {package_name} package doesn't contain any bindings")]
    NoBindings { package_name: String },
}

#[derive(graphql_client::GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/get_bindings.graphql",
    response_derives = "Debug,PartialEq"
)]
struct GetBindingsQuery;

/// Get the link to the bindings for a package.
///
/// If the package contains multiple modules with bindings, the `module`
/// argument is used to pick the correct one.
pub fn link_to_package_bindings(
    package_name: &str,
    version: Option<&str>,
    language: Language,
) -> Result<String, Error> {
    let q = GetBindingsQuery::build_query(get_bindings_query::Variables {
        name: package_name.to_string(),
        version: version.map(|s| s.to_string()),
    });

    let config =
        crate::config::Config::from_file().map_err(|e| Error::Query(anyhow::anyhow!("{e}")))?;

    let get_bindings_query::ResponseData { package_version } =
        crate::graphql::execute_query(&q).map_err(Error::Query)?;
    let get_bindings_query::GetBindingsQueryPackageVersion { bindings, version } = package_version
        .ok_or_else(|| Error::UnknownPackage {
            name: package_name.to_string(),
            registry: config.registry.get_current_registry(),
        })?;

    if bindings.is_empty() {
        return Err(Error::NoBindings {
            package_name: package_name.to_string(),
        });
    }

    let chosen_language = match language {
        Language::JavaScript => get_bindings_query::ProgrammingLanguage::JAVASCRIPT,
        Language::Python => get_bindings_query::ProgrammingLanguage::PYTHON,
    };

    let bindings = bindings
        .into_iter()
        .flatten()
        .find(|b| b.language == chosen_language)
        .ok_or_else(|| Error::MissingBindings {
            package_name: package_name.to_string(),
            version,
            language,
        })?;

    Ok(bindings.url)
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Language {
    JavaScript,
    Python,
}

impl Display for Language {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Language::JavaScript => "JavaScript".fmt(f),
            Language::Python => "Python".fmt(f),
        }
    }
}

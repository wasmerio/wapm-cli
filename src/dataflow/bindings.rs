use std::{
    collections::BTreeMap,
    fmt::{self, Display, Formatter},
};

use graphql_client::GraphQLQuery;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Query(anyhow::Error),
    #[error("Package not found in the registry {registry:?}: {name}")]
    UnknownPackage { name: String, registry: String },
    #[error("{package_name} v{version} doesn't have any {language} bindings for {module}")]
    MissingBindings {
        package_name: String,
        version: String,
        module: String,
        language: Language,
    },
    #[error("The {package_name} package doesn't contain any bindings")]
    NoBindings { package_name: String },
    #[error("The {package_name} package contains bindings for multiple modules. Please choose one of {available_modules:?}")]
    MultipleBindings {
        package_name: String,
        available_modules: Vec<String>,
    },
}

#[derive(graphql_client::GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/get_bindings.graphql",
    response_derives = "Debug"
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
    module: Option<&str>,
) -> Result<String, Error> {
    let q = GetBindingsQuery::build_query(get_bindings_query::Variables {
        name: package_name.to_string(),
        version: version.map(|s| s.to_string()),
    });

    let config = crate::config::Config::from_file()
        .map_err(|e| Error::Query(anyhow::anyhow!("{e}")))?;
    
    let get_bindings_query::ResponseData { package_version } =
        crate::graphql::execute_query(&q).map_err(Error::Query)?;
    let get_bindings_query::GetBindingsQueryPackageVersion { bindings, version } = package_version
        .ok_or_else(|| Error::UnknownPackage {
            name: package_name.to_string(),
            registry: config.registry.get_current_registry(),
        })?;

    let mut candidates: BTreeMap<_, _> = bindings.into_iter()
        .filter_map(|b| b)
        .filter_map(|bindings| match (language, bindings.on) {
            (Language::JavaScript, get_bindings_query::GetBindingsQueryPackageVersionBindingsOn::PackageVersionNPMBinding(b)) => Some((
                bindings.module,
                b.npm_default_install_package_name,
        )),
            (Language::Python, get_bindings_query::GetBindingsQueryPackageVersionBindingsOn::PackageVersionPythonBinding(b)) => Some((
                bindings.module,
                b.python_default_install_package_name,
        )),
            _ => None,
        })
        .collect();

    if candidates.is_empty() {
        return Err(Error::NoBindings {
            package_name: package_name.to_string(),
        });
    }

    match module {
        Some(m) => candidates.remove(m).ok_or_else(|| Error::MissingBindings {
            package_name: package_name.to_string(),
            version,
            module: m.to_string(),
            language,
        }),
        None if candidates.len() == 1 => {
            let (_, url) = candidates.into_iter().next().expect("");
            Ok(url)
        }
        None => {
            let available_modules = candidates.into_iter().map(|(module, _)| module).collect();
            Err(Error::MultipleBindings {
                package_name: package_name.to_string(),
                available_modules,
            })
        }
    }
}

#[derive(Debug, Copy, Clone)]
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

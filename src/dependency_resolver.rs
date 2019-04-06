use crate::graphql::execute_query;
use crate::manifest::Manifest;
use graphql_client::*;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/get_package_version_manifest.graphql",
    response_derives = "Debug"
)]
struct GetPackageVersionManifestQuery;

#[derive(Clone, Debug)]
pub struct Dependency {
    pub name: String,
    pub manifest: Manifest,
    pub download_url: String,
}

pub trait DependencyResolver {
    fn resolve(&self, pkg_name: &str, pkg_version: &str) -> Result<Dependency, failure::Error>;
}

#[cfg(test)]
pub struct TestResolver(pub std::collections::BTreeMap<(String, String), Dependency>);

#[cfg(test)]
impl DependencyResolver for TestResolver {
    fn resolve(&self, pkg_name: &str, pkg_version: &str) -> Result<Dependency, failure::Error> {
        let key = (pkg_name.to_string(), pkg_version.to_string());
        ensure!(self.0.contains_key(&key), "pkg not found");
        let dependency: Dependency = self.0.get(&key).unwrap().clone();
        Ok(dependency)
    }
}

pub struct RegistryResolver;

impl DependencyResolver for RegistryResolver {
    fn resolve(&self, pkg_name: &str, pkg_version: &str) -> Result<Dependency, failure::Error> {
        let q = GetPackageVersionManifestQuery::build_query(
            get_package_version_manifest_query::Variables {
                name: pkg_name.to_string(),
                version: pkg_version.to_string(),
            },
        );
        let response: get_package_version_manifest_query::ResponseData = execute_query(&q)?;
        let package = response
            .package
            .ok_or(DependencyResolverError::MissingDependency(
                pkg_name.to_string(),
                pkg_version.to_string(),
            ))?;
        let manifest_string: String = package.manifest;
        let download_url: String = package.distribution.download_url;
        let manifest: Manifest = toml::from_str(&manifest_string)?;
        let name = package.package.name;
        let dependency = Dependency {
            name,
            manifest,
            download_url,
        };
        Ok(dependency)
    }
}

#[derive(Debug, Fail)]
enum DependencyResolverError {
    #[fail(display = "Package not found in the registry: {}@{}", _0, _1)]
    MissingDependency(String, String),
}

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

pub trait DependencyResolver {
    fn resolve(&self, pkg_name: &str, pkg_version: &str) -> Result<Manifest, failure::Error>;
}

#[cfg(test)]
pub struct TestResolver(pub std::collections::BTreeMap<(String, String), Manifest>);

#[cfg(test)]
impl DependencyResolver for TestResolver {
    fn resolve(&self, pkg_name: &str, pkg_version: &str) -> Result<Manifest, failure::Error> {
        let key = (pkg_name.to_string(), pkg_version.to_string());
        ensure!(self.0.contains_key(&key), "pkg not found");
        Ok(self.0.get(&key).map(|l| l.clone()).unwrap())
    }
}

pub struct RegistryResolver;

impl DependencyResolver for RegistryResolver {
    fn resolve(&self, pkg_name: &str, pkg_version: &str) -> Result<Manifest, failure::Error> {
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
        let manifest: Manifest = toml::from_str(&manifest_string)?;
        Ok(manifest)
    }
}

#[derive(Debug, Fail)]
enum DependencyResolverError {
    #[fail(display = "Package not found in the registry: {}@{}", _0, _1)]
    MissingDependency(String, String),
}

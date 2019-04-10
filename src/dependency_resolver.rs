use crate::graphql::execute_query;
use crate::manifest::Manifest;
use graphql_client::*;
use std::collections::BTreeMap;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/get_packages.graphql",
    response_derives = "Debug"
)]
struct GetPackagesQuery;

#[derive(Clone, Debug)]
pub struct Dependency {
    pub name: String,
    pub version: String,
    pub manifest: Manifest,
    pub download_url: String,
    pub is_top_level_dependency: bool,
}

pub trait PackageRegistryLike {
    //    fn resolve(&self, pkg_name: &str, pkg_version: &str) -> Result<Dependency, failure::Error>;
    fn get_all_dependencies<'a>(
        &'a mut self,
        root_pkg_name: &'a str,
        root_pkg_version: &'a str,
        root_dependencies: Vec<(&'a str, &'a str)>,
    ) -> Result<Vec<&'a Dependency>, failure::Error>;
}

#[cfg(test)]
pub struct TestRegistry(pub BTreeMap<&'static str, Vec<Dependency>>);

#[cfg(test)]
impl PackageRegistryLike for TestRegistry {
    fn get_all_dependencies<'a>(
        &'a mut self,
        _root_pkg_name: &'a str,
        _root_pkg_version: &'a str,
        root_dependencies: Vec<(&'a str, &'a str)>,
    ) -> Result<Vec<&'a Dependency>, failure::Error> {
        // for now, only fetch root dependencies
        let mut dependencies = vec![];
        for (package_name, package_version) in root_dependencies {
            match self.0.get(package_name) {
                Some(versions) => {
                    let version = versions
                        .iter()
                        .find(|v| v.version.as_str() == package_version);
                    let dependency = version.ok_or(DependencyResolverError::MissingDependency(
                        package_name.to_string(),
                        package_version.to_string(),
                    ))?;
                    dependencies.push(dependency);
                }
                None => {
                    return Err(DependencyResolverError::MissingDependency(
                        package_name.to_string(),
                        package_version.to_string(),
                    )
                    .into());
                }
            }
        }
        Ok(dependencies)
    }
}

pub struct PackageRegistry(pub BTreeMap<String, Vec<Dependency>>);

impl PackageRegistry {
    pub fn new() -> Self {
        PackageRegistry(BTreeMap::new())
    }

    fn sync_packages(&mut self, package_names: Vec<String>) -> Result<(), failure::Error> {
        let q = GetPackagesQuery::build_query(get_packages_query::Variables {
            names: Some(package_names),
        });
        let response: get_packages_query::ResponseData = execute_query(&q)?;
        for p in response.package.into_iter().map(Option::unwrap) {
            let package_name: String = p.name;
            let versions = p
                .versions
                .unwrap_or(vec![])
                .into_iter()
                .filter_map(|o| o)
                .map(|v| {
                    v.package
                        .versions
                        .unwrap_or(vec![])
                        .into_iter()
                        .filter_map(|o| o)
                })
                .flatten();
            let mut package_versions = vec![];
            for v in versions {
                let manifest: Manifest = toml::from_str(&v.manifest)?;
                package_versions.push(Dependency {
                    name: package_name.clone(),
                    version: v.version,
                    manifest,
                    download_url: v.distribution.download_url,
                    is_top_level_dependency: true, // TODO fix this
                });
            }
            self.0.insert(package_name, package_versions);
        }
        Ok(())
    }
}

impl PackageRegistryLike for PackageRegistry {
    fn get_all_dependencies<'a>(
        &'a mut self,
        _root_pkg_name: &'a str,
        _root_pkg_version: &'a str,
        root_dependencies: Vec<(&'a str, &'a str)>,
    ) -> Result<Vec<&'a Dependency>, failure::Error> {
        // for now, only fetch root dependencies
        let package_names: Vec<String> =
            root_dependencies.iter().map(|t| t.0.to_string()).collect();
        // update local map of packages
        self.sync_packages(package_names)?;

        let mut dependencies = vec![];

        for (package_name, package_version) in root_dependencies {
            match self.0.get(package_name) {
                Some(versions) => {
                    let version = versions
                        .iter()
                        .find(|v| v.version.as_str() == package_version);
                    let dependency = version.ok_or(DependencyResolverError::MissingDependency(
                        package_name.to_string(),
                        package_version.to_string(),
                    ))?;
                    dependencies.push(dependency);
                }
                None => {
                    return Err(DependencyResolverError::MissingDependency(
                        package_name.to_string(),
                        package_version.to_string(),
                    )
                    .into());
                }
            }
        }

        Ok(dependencies)
    }
}

#[derive(Debug, Fail)]
enum DependencyResolverError {
    #[fail(display = "Package not found in the registry: {}@{}", _0, _1)]
    MissingDependency(String, String),
    //    #[fail(display = "Dependency version must be a string. Package name: {}.", _0)]
    //    DependencyVersionMustBeString(String),
}

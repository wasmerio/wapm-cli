use crate::bonjour::PackageKey::WapmPackage;
use crate::bonjour::WapmPackageKey;
use crate::cfg_toml::manifest::{Manifest, PACKAGES_DIR_NAME};
use crate::graphql::execute_query;
use crate::install::install_package;
use crate::util::get_package_namespace_and_name;
use graphql_client::*;
use std::collections::btree_map::BTreeMap;
use std::collections::hash_map::HashMap;
use std::collections::hash_set::HashSet;
use std::env;
use std::path::PathBuf;
use std::borrow::Cow;

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
    pub wapm_package_directory: PathBuf,
}

impl Dependency {
    pub fn new<S1: AsRef<str>, S2: AsRef<str>, S3: AsRef<str>>(
        name: S1,
        version: S2,
        manifest: Manifest,
        download_url: S3,
    ) -> Self {
        let (namespace, unqualified_pkg_name) =
            get_package_namespace_and_name(name.as_ref()).unwrap();
        let pkg_dir = format!("{}@{}", unqualified_pkg_name, version.as_ref());
        let wapm_package_directory: PathBuf =
            [PACKAGES_DIR_NAME, namespace, &pkg_dir].iter().collect();
        Dependency {
            name: name.as_ref().to_string(),
            version: version.as_ref().to_string(),
            manifest,
            download_url: download_url.as_ref().to_string(),
            is_top_level_dependency: true, // TODO fix this
            wapm_package_directory,
        }
    }
}

pub trait PackageRegistryLike {
    fn get_all_dependencies<'a>(
        &'a mut self,
        root_dependencies: Vec<(&'a str, &'a str)>,
    ) -> Result<Vec<&'a Dependency>, failure::Error>;
}

#[cfg(test)]
pub struct TestRegistry(pub BTreeMap<&'static str, Vec<Dependency>>);

#[cfg(test)]
impl PackageRegistryLike for TestRegistry {
    fn get_all_dependencies<'a>(
        &'a mut self,
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

#[derive(Debug)]
pub struct PackageRegistry<'a> {
    pub registry_data: HashMap<WapmPackageKey<'a>, String>,
}

impl<'a> PackageRegistry<'a> {
    pub fn new() -> Self {
        PackageRegistry {
            registry_data: HashMap::new(),
        }
    }

    pub fn new_from_pkg_ids(added_set: Vec<WapmPackageKey<'a>>) -> Vec<Dependency> {
        unimplemented!()
    }

    fn get_response(added_pkgs: Vec<WapmPackageKey<'a>>) -> get_packages_query::ResponseData {
        let mut set: HashSet<WapmPackageKey<'a>> = added_pkgs.into_iter().collect();
        let names = set.iter().map(|k| k.name.to_string()).collect();
        let q = GetPackagesQuery::build_query(get_packages_query::Variables { names });
        execute_query(&q).unwrap()
    }

    pub fn better_sync_packages(
        added_pkgs: Vec<WapmPackageKey<'a>>,
    ) -> Result<Vec<(WapmPackageKey<'a>, String)>, failure::Error> {
        let response = Self::get_response(added_pkgs.clone());
        let results: Vec<(WapmPackageKey<'a>, String)> = response
            .package
            .into_iter()
            .filter_map(|p| p)
            .filter_map(|p| {
                let versions = p.versions.unwrap_or(vec![]);
                let name = p.name;
                Some((name, versions))
            })
            .map(|(n, vs)| {
                vs.into_iter()
                    .filter_map(|o| o)
                    .map(|v| {
                        let version = v.version;
                        let download_url = v.distribution.download_url;
                        (n.clone(), version, download_url)
                    })
                    .collect::<Vec<_>>()
            })
            .flatten()
            .filter_map(|(n, v, d)| {
                let key = added_pkgs.iter().find(|x| x.name == n && x.version == v);
                key.map(|k| (k.clone(), d))
            })
            .collect();
        Ok(results)
    }

    fn sync_packages(
        &mut self,
        package_names_and_versions: &[(&str, &str)],
    ) -> Result<(), failure::Error> {
        unimplemented!();
        //        let package_names: Vec<String> = package_names_and_versions
        //            .iter()
        //            .map(|t| t.0.to_string())
        //            .collect();
        //        let q = GetPackagesQuery::build_query(get_packages_query::Variables {
        //            names: package_names,
        //        });
        //        let response: get_packages_query::ResponseData = execute_query(&q)?;
        //        for (i, pkg) in response.package.into_iter().enumerate() {
        //            let p = pkg.ok_or(DependencyResolverError::MissingDependency(
        //                package_names_and_versions[i].0.to_string(),
        //                package_names_and_versions[i].1.to_string(),
        //            ))?;
        //            let package_name: String = p.name;
        //            let versions = p
        //                .versions
        //                .unwrap_or_default()
        //                .into_iter()
        //                .filter_map(|o| o)
        //                .map(|v| {
        //                    v.package
        //                        .versions
        //                        .unwrap_or_default()
        //                        .into_iter()
        //                        .filter_map(|o| o)
        //                })
        //                .flatten();
        //
        //            // skip old manifests that are no longer valid
        //            let package_versions: Vec<Dependency> = versions
        //                .into_iter()
        //                .map(|v| {
        //                    (
        //                        toml::from_str::<Manifest>(&v.manifest),
        //                        v.version,
        //                        v.distribution.download_url,
        //                    )
        //                })
        //                .filter(|v| v.0.is_ok())
        //                .map(|v| Dependency::new(&package_name, v.1.as_str(), v.0.unwrap(), v.2.as_str()))
        //                .collect();
        //
        //            // if the dependency is a global package, we insert two records:
        //            // _/package -> Versions
        //            // package -> Versions
        //            // Lookups can use either name representation
        //            self.0
        //                .insert(package_name.clone(), package_versions.clone());
        //            if package_name.starts_with("_/") {
        //                let name = package_name[2..].to_string();
        //                self.0.insert(name, package_versions);
        //            }
        //        }
        //        Ok(())
    }

    pub fn get_all_dependencies(
        &'a mut self,
        root_dependencies: Vec<(Cow<'a, &str>, Cow<'a, &str>)>,
    ) -> Result<Vec<&'a Dependency>, failure::Error> {
        unimplemented!();
        // return early if there are no dependencies to resolve
        if root_dependencies.is_empty() {
            return Ok(vec![]);
        }
        // for now, only fetch root dependencies
        // update local map of packages
        //        self.sync_packages(&root_dependencies)?;

        //        Self::better_sync_packages()
        unimplemented!();
        //
        //        let mut dependencies = vec![];
        //
        //        for (package_name, package_version) in root_dependencies {
        //            match self.0.get(package_name) {
        //                Some(versions) => {
        //                    let version = versions
        //                        .iter()
        //                        .find(|v| v.version.as_str() == package_version);
        //                    let dependency = version.ok_or(DependencyResolverError::MissingDependency(
        //                        package_name.to_string(),
        //                        package_version.to_string(),
        //                    ))?;
        //                    dependencies.push(dependency);
        //                }
        //                None => {
        //                    return Err(DependencyResolverError::MissingDependency(
        //                        package_name.to_string(),
        //                        package_version.to_string(),
        //                    )
        //                    .into());
        //                }
        //            }
        //        }
        //
        //        let cwd = env::current_dir()?;
        //        for dependency in dependencies.iter().cloned() {
        //            let dependency: &Dependency = dependency;
        //            if !dependency.wapm_package_directory.exists() {
        //                info!("Installing {}@{}", dependency.name, dependency.version);
        //                install_package(dependency, &cwd)?;
        //            } else {
        //                debug!(
        //                    "Package {}@{} already exists; doing nothing",
        //                    dependency.name, dependency.version
        //                );
        //            }
        //        }
        //        Ok(dependencies)
    }
}

#[derive(Debug, Fail)]
enum DependencyResolverError {
    #[fail(display = "Package not found in the registry: {}@{}", _0, _1)]
    MissingDependency(String, String),
}

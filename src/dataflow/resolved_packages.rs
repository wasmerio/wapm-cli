use crate::dataflow::added_packages::AddedPackages;
use crate::dataflow::{PackageKey, WapmPackageKey, WapmPackageRange};
use crate::graphql::execute_query;
use graphql_client::*;
use semver::Version;
use std::borrow::Cow::Owned;
use std::collections::hash_map::HashMap;
use std::collections::hash_set::HashSet;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/get_packages.graphql",
    response_derives = "Debug"
)]
struct GetPackagesQuery;

#[derive(Clone, Debug, Fail)]
pub enum Error {
    #[fail(display = "There was a problem resolve dependencies. {}", _0)]
    CouldNotResolvePackages(String),
}

/// Struct containing wapm registry resolved packages. This is realized as a pairing of wapm.io keys
/// and download URLs.
#[derive(Clone, Debug, Default)]
pub struct ResolvedPackages<'a> {
    pub packages: Vec<(WapmPackageKey<'a>, String)>,
}

impl<'a> ResolvedPackages<'a> {
    /// Consume changed manifest packages and produce keys with download urls. Will query the registry
    /// for the download urls.
    fn new<Resolver>(packages: HashSet<PackageKey<'a>>) -> Result<Self, Error>
    where
        Resolver: Resolve<'a>,
    {
        let wapm_pkgs: Vec<PackageKey> = packages
            .into_iter()
            .filter_map(|key| match key {
                PackageKey::WapmPackage(_) => Some(key),
                PackageKey::WapmPackageRange(_) => Some(key),
            })
            .collect();
        // return early if no packages to resolve
        if wapm_pkgs.is_empty() {
            return Ok(Self::default());
        }
        let packages = Resolver::sync_packages(wapm_pkgs)
            .map_err(|e| Error::CouldNotResolvePackages(e.to_string()))?;
        Ok(Self { packages })
    }

    pub fn new_from_added_packages<Resolver>(
        added_packages: AddedPackages<'a>,
    ) -> Result<Self, Error>
    where
        Resolver: Resolve<'a>,
    {
        Self::new::<Resolver>(added_packages.packages)
    }
}

/// A Resolve trait to enable testing and dependency injection
pub trait Resolve<'a> {
    fn sync_packages(
        added_packages: Vec<PackageKey<'a>>,
    ) -> Result<Vec<(WapmPackageKey<'a>, String)>, Error>;
}

pub struct RegistryResolver;

impl<'a> RegistryResolver {
    fn get_response(added_pkgs: Vec<PackageKey<'a>>) -> get_packages_query::ResponseData {
        let names = added_pkgs
            .into_iter()
            .map(|key| match key {
                PackageKey::WapmPackageRange(WapmPackageRange { name, .. }) => name.to_string(),
                PackageKey::WapmPackage(WapmPackageKey { name, .. }) => name.to_string(),
            })
            .collect();
        let q = GetPackagesQuery::build_query(get_packages_query::Variables { names });
        execute_query(&q).unwrap()
    }
}

/// The Registry Resolver will resolve dependencies on a wapm.io server
impl<'a> Resolve<'a> for RegistryResolver {
    /// This gross function queries the GraphQL server. See the schema in `/graphql/queries/get_packages.graphql`
    fn sync_packages(
        added_packages: Vec<PackageKey<'a>>,
    ) -> Result<Vec<(WapmPackageKey<'a>, String)>, Error> {
        // fetch data from graphql server
        let response = Self::get_response(added_packages.clone());
        let all_packages_and_download_urls: Vec<(String, Version, String)> = response
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
            .map(|(name, version, download_url)| {
                Version::parse(&version)
                    .map(|version| (name, version, download_url))
                    .map_err(|e| Error::CouldNotResolvePackages(e.to_string()))
            })
            .collect::<Result<Vec<(_, _, _)>, Error>>()?;

        // lookup by exact package key
        let exact_package_lookup: HashMap<_, _> = all_packages_and_download_urls
            .iter()
            .cloned()
            .map(|(name, version, download_url)| {
                (
                    WapmPackageKey {
                        name: Owned(name),
                        version,
                    },
                    download_url,
                )
            })
            .collect();

        // lookup versions by name, used for matching package version ranges
        let mut package_versions_lookup: HashMap<String, Vec<Version>> = HashMap::new();
        for (name, version, _) in all_packages_and_download_urls {
            let versions = package_versions_lookup.entry(name).or_default();
            versions.push(version);
        }

        // filter all the package-versions + download_urls by exact version or version range
        let packages_and_download_urls: Vec<(WapmPackageKey, String)> = added_packages
            .into_iter()
            .filter_map(|added_package| match added_package {
                // if exact, then use the lookup table
                PackageKey::WapmPackage(wapm_package_key) => exact_package_lookup
                    .get(&wapm_package_key)
                    .map(|d| (wapm_package_key, d.clone())),
                // if a range, then filter by the requirements, and find the max version
                PackageKey::WapmPackageRange(range) => {
                    let matching_version: Option<Version> = package_versions_lookup
                        .get(range.name.as_ref())
                        .map(|versions| {
                            let max_version: Option<Version> = versions
                                .iter()
                                .cloned()
                                .filter(|v| range.version_req.matches(v))
                                .max(); // get the max version number after filtering by version requirement
                            max_version
                        })
                        .unwrap_or_default();
                    // join the key with the download url by using the package-key lookup table
                    let key_and_download_url: Option<(WapmPackageKey, String)> = matching_version
                        .map(|version| {
                            let key = WapmPackageKey {
                                name: range.name,
                                version,
                            };
                            let download_url = exact_package_lookup.get(&key);
                            download_url.map(|dl| (key, dl.clone()))
                        })
                        .unwrap_or_default();
                    key_and_download_url
                }
            })
            .collect();
        Ok(packages_and_download_urls)
    }
}

#[cfg(test)]
mod test {
    use crate::dataflow::added_packages::AddedPackages;
    use crate::dataflow::resolved_packages::{Error, Resolve, ResolvedPackages};
    use crate::dataflow::{PackageKey, WapmPackageKey, WapmPackageRange};
    use std::collections::HashSet;

    struct TestResolver;

    /// A test resolver that does not resolve the "baz" and "bar" packages but contains everything else.
    impl<'a> Resolve<'a> for TestResolver {
        fn sync_packages(
            added_packages: Vec<PackageKey<'a>>,
        ) -> Result<Vec<(WapmPackageKey<'a>, String)>, Error> {
            Ok(added_packages
                .into_iter()
                .filter(|k| {
                    match k {
                        PackageKey::WapmPackage(WapmPackageKey { name, .. }) => {
                            name != "_/bar" && // simulate non-existent packages
                                name != "_/baz"
                        }
                        PackageKey::WapmPackageRange(WapmPackageRange { name, .. }) => {
                            name != "_/bar" && // simulate non-existent packages
                                name != "_/baz"
                        }
                    }
                })
                .map(|k| match k {
                    PackageKey::WapmPackage(WapmPackageKey { name, .. }) => (
                        WapmPackageKey {
                            name,
                            version: semver::Version::new(0, 0, 0),
                        },
                        "url".to_string(),
                    ),
                    PackageKey::WapmPackageRange(WapmPackageRange { name, .. }) => (
                        WapmPackageKey {
                            name,
                            version: semver::Version::new(0, 0, 0),
                        },
                        "url".to_string(),
                    ),
                })
                .collect())
        }
    }

    #[test]
    fn test_resolve() {
        let package_key_1 =
            PackageKey::new_registry_package("_/foo", semver::Version::new(1, 0, 0));
        let mut packages_set = HashSet::new();
        packages_set.insert(package_key_1);
        let added_packages = AddedPackages {
            packages: packages_set,
        };
        let resolve_packages =
            ResolvedPackages::new_from_added_packages::<TestResolver>(added_packages).unwrap();
        assert_eq!(1, resolve_packages.packages.len());
    }

    #[test]
    fn test_resolve_missing_packages() {
        let package_key_1 =
            PackageKey::new_registry_package("_/foo", semver::Version::new(1, 0, 0));
        let package_key_2 =
            PackageKey::new_registry_package("_/baz", semver::Version::new(1, 0, 0));
        let package_key_3 =
            PackageKey::new_registry_package("_/bar", semver::Version::new(1, 0, 0));
        let mut packages_set = HashSet::new();
        packages_set.insert(package_key_1);
        packages_set.insert(package_key_2);
        packages_set.insert(package_key_3);
        let added_packages = AddedPackages {
            packages: packages_set,
        };
        let resolve_packages =
            ResolvedPackages::new_from_added_packages::<TestResolver>(added_packages).unwrap();
        assert_eq!(1, resolve_packages.packages.len());
    }

    #[test]
    fn test_resolve_missing_packages_with_ranges() {
        let package_key_1 = PackageKey::new_registry_package_range(
            "_/foo",
            semver::VersionReq::parse("^1").unwrap(),
        );
        let package_key_2 = PackageKey::new_registry_package_range(
            "_/baz",
            semver::VersionReq::parse("*").unwrap(),
        );
        let mut packages_set = HashSet::new();
        packages_set.insert(package_key_1);
        packages_set.insert(package_key_2);
        let added_packages = AddedPackages {
            packages: packages_set,
        };
        let resolve_packages =
            ResolvedPackages::new_from_added_packages::<TestResolver>(added_packages).unwrap();
        assert_eq!(1, resolve_packages.packages.len());
        resolve_packages
            .packages
            .into_iter()
            .find(|(p, _s)| p.name == "_/foo")
            .unwrap();
    }
}

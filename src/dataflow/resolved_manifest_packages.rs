use crate::dataflow::changed_manifest_packages::ChangedManifestPackages;
use crate::dataflow::{PackageKey, WapmPackageKey};
use crate::graphql::execute_query;
use graphql_client::*;
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
#[derive(Clone, Debug)]
pub struct ResolvedManifestPackages<'a> {
    pub packages: Vec<(WapmPackageKey<'a>, String)>,
}

impl<'a> ResolvedManifestPackages<'a> {
    /// Consume changed manifest packages and produce keys with download urls. Will query the registry
    /// for the download urls.
    pub fn new<Resolver>(manifest_data: ChangedManifestPackages<'a>) -> Result<Self, Error>
    where
        Resolver: Resolve<'a>,
    {
        let wapm_pkgs = manifest_data
            .packages
            .into_iter()
            .filter_map(|k| match k {
                PackageKey::WapmPackage(k) => Some(k),
                _ => panic!("Non-wapm registry keys are not supported."),
            })
            .collect();
        let packages = Resolver::sync_packages(wapm_pkgs)
            .map_err(|e| Error::CouldNotResolvePackages(e.to_string()))?;
        Ok(Self { packages })
    }
}

/// A Resolve trait to enable testing and dependency injection
pub trait Resolve<'a> {
    fn sync_packages(
        added_packages: Vec<WapmPackageKey<'a>>,
    ) -> Result<Vec<(WapmPackageKey<'a>, String)>, Error>;
}

pub struct RegistryResolver;

impl<'a> RegistryResolver {
    fn get_response(added_pkgs: Vec<WapmPackageKey<'a>>) -> get_packages_query::ResponseData {
        let set: HashSet<WapmPackageKey<'a>> = added_pkgs.into_iter().collect();
        let names = set.into_iter().map(|k| k.name.to_string()).collect();
        let q = GetPackagesQuery::build_query(get_packages_query::Variables { names });
        execute_query(&q).unwrap()
    }
}

/// The Registry Resolver will resolve dependencies on a wapm.io server
impl<'a> Resolve<'a> for RegistryResolver {
    /// This gross function queries the GraphQL server. See the schema in `/graphql/queries/get_packages.grapql`
    fn sync_packages(
        added_packages: Vec<WapmPackageKey<'a>>,
    ) -> Result<Vec<(WapmPackageKey<'a>, String)>, Error> {
        let response = Self::get_response(added_packages.clone());
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
            // This is hack to allow for matching on the shorthand notation of global packages
            // e.g. "_/sqlite" and "sqlite" are equivalent
            .filter_map(|(n, v, d)| {
                let key = added_packages.iter().find(|k| match k.name.find('/') {
                    Some(_) => k.name == n && k.version == v,
                    _ => k.name == &n[2..] && k.version == v,
                });
                key.map(|k| (k.clone(), d))
            })
            .collect();
        Ok(results)
    }
}

#[cfg(test)]
mod test {
    use crate::dataflow::changed_manifest_packages::ChangedManifestPackages;
    use crate::dataflow::resolved_manifest_packages::{Error, Resolve, ResolvedManifestPackages};
    use crate::dataflow::{PackageKey, WapmPackageKey};
    use std::collections::HashSet;

    struct TestResolver;

    /// A test resolver that does not resolve the "baz" and "bar" packages but contains everything else.
    impl<'a> Resolve<'a> for TestResolver {
        fn sync_packages(
            added_packages: Vec<WapmPackageKey<'a>>,
        ) -> Result<Vec<(WapmPackageKey<'a>, String)>, Error> {
            Ok(added_packages
                .into_iter()
                .filter(|k| {
                    k.name != "_/bar" && // simulate non-existent packages
                    k.name != "_/baz"
                })
                .map(|k| (k, "url".to_string()))
                .collect())
        }
    }

    #[test]
    fn test_resolve() {
        let package_key_1 = PackageKey::new_registry_package("_/foo", "1.0.0");
        let mut packages_set = HashSet::new();
        packages_set.insert(package_key_1);
        let changed_packages = ChangedManifestPackages {
            packages: packages_set,
        };
        let resolve_packages =
            ResolvedManifestPackages::new::<TestResolver>(changed_packages).unwrap();
        assert_eq!(1, resolve_packages.packages.len());
    }

    #[test]
    fn test_resolve_missing_packages() {
        let package_key_1 = PackageKey::new_registry_package("_/foo", "1.0.0");
        let package_key_2 = PackageKey::new_registry_package("_/baz", "1.0.0");
        let package_key_3 = PackageKey::new_registry_package("_/bar", "1.0.0");
        let mut packages_set = HashSet::new();
        packages_set.insert(package_key_1);
        packages_set.insert(package_key_2);
        packages_set.insert(package_key_3);
        let changed_packages = ChangedManifestPackages {
            packages: packages_set,
        };
        let resolve_packages =
            ResolvedManifestPackages::new::<TestResolver>(changed_packages).unwrap();
        assert_eq!(1, resolve_packages.packages.len());
    }
}

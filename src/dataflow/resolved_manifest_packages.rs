use crate::dataflow::changed_manifest_packages::ChangedManifestPackages;
use crate::dataflow::{Error, PackageKey, WapmPackageKey};
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

/// Struct containing wapm registry resolved packages. This is realized as a pairing of wapm.io keys
/// and download URLs.
#[derive(Clone, Debug)]
pub struct ResolvedManifestPackages<'a> {
    pub packages: Vec<(WapmPackageKey<'a>, String)>,
}

impl<'a> ResolvedManifestPackages<'a> {
    /// Consume changed manifest packages and produce keys with download urls. Will query the registry
    /// for the download urls.
    pub fn new(manifest_data: ChangedManifestPackages<'a>) -> Result<Self, Error> {
        let wapm_pkgs = manifest_data
            .packages
            .into_iter()
            .filter_map(|k| match k {
                PackageKey::WapmPackage(k) => Some(k),
                _ => panic!("Non-wapm registry keys are not supported."),
            })
            .collect();
        let packages: Vec<_> = Self::sync_packages(wapm_pkgs)
            .map_err(|e| Error::InstallError(e.to_string()))?;
        Ok(Self { packages })
    }

    fn get_response(added_pkgs: Vec<WapmPackageKey<'a>>) -> get_packages_query::ResponseData {
        let set: HashSet<WapmPackageKey<'a>> = added_pkgs.into_iter().collect();
        let names = set.into_iter().map(|k| k.name.to_string()).collect();
        let q = GetPackagesQuery::build_query(get_packages_query::Variables { names });
        execute_query(&q).unwrap()
    }

    fn sync_packages(
        added_packages: Vec<WapmPackageKey<'a>>,
    ) -> Result<Vec<(WapmPackageKey<'a>, String)>, failure::Error> {
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

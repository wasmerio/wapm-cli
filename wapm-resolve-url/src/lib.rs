use url::Url;
use graphql_client::GraphQLQuery;

mod graphql;
#[cfg(not(target_arch = "wasm32"))]
mod proxy;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/query-url-of-file-targz.graphql",
    response_derives = "Debug"
)]
pub struct GetPackageQueryTarGz;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/query-url-of-file-pirita.graphql",
    response_derives = "Debug"
)]
pub struct GetPackageQueryPirita;

#[cfg(target_os = "wasi")]
pub fn whoami_distro() -> String {
    whoami::os().to_lowercase()
}

#[cfg(not(target_os = "wasi"))]
pub fn whoami_distro() -> String {
    whoami::distro().to_lowercase()
}

pub fn get_current_wapm_registry() -> Option<Url> {
    let command = std::process::Command::new("wapm")
    .arg("config")
    .arg("get")
    .arg("registry.url")
    .output()
    .ok()?;
    Some(Url::parse(std::str::from_utf8(&command.stdout).ok()?).ok()?)
}

pub fn get_tar_gz_url_of_package(registry: &Url, package_id: &str, version: Option<&str>) -> Option<(Url, String)> {

    let q = GetPackageQueryTarGz::build_query(get_package_query_tar_gz::Variables {
        name: package_id.to_string(),
    });
    let all_package_versions: get_package_query_tar_gz::ResponseData = crate::graphql::execute_query(registry, &q).ok()?;

    match version {
        Some(specific) => {
            let last_package = all_package_versions.package?.versions?;
            
            let last_package = last_package
            .iter()
            .filter_map(|v| v.as_ref())
            .filter(|v| v.version == specific)
            .next()?;

            Url::parse(&last_package.distribution.download_url)
            .ok()
            .map(|u| (u, last_package.version.clone()))
        },
        None => {
            let last_version = all_package_versions.package?.last_version?;
            Url::parse(&last_version.distribution.download_url)
            .ok()
            .map(|u| (u, last_version.version.clone()))
        },
    }
}

pub fn get_pirita_url_of_package(registry: &Url, package_id: &str, version: Option<&str>) -> Option<(Url, String)> {
    
    let q = GetPackageQueryPirita::build_query(get_package_query_pirita::Variables {
        name: package_id.to_string(),
    });
    let all_package_versions: get_package_query_pirita::ResponseData = crate::graphql::execute_query(registry, &q).ok()?;

    match version {
        Some(specific) => {
            let last_package = all_package_versions.package?.versions?;

            let last_package = last_package
            .iter()
            .filter_map(|v| v.as_ref())
            .filter(|v| v.version == specific)
            .next()?;

            Url::parse(&last_package.distribution.pirita_download_url.as_ref().map(|s| s.as_str())?)
            .ok()
            .map(|u| (u, last_package.version.clone()))
        },
        None =>{
            let last_version = all_package_versions.package?.last_version?;
            Url::parse(&last_version.distribution.pirita_download_url.as_ref().map(|s| s.as_str())?)
            .ok()
            .map(|u| (u, last_version.version.clone()))
        },
    }
}

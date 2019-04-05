use crate::graphql::execute_query_modifier;
use crate::manifest::Manifest;

use graphql_client::*;
use std::fs;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/publish_package.graphql",
    response_derives = "Debug"
)]
struct PublishPackageMutation;

pub fn publish() -> Result<(), failure::Error> {
    let manifest = Manifest::find_in_current_directory()?;
    let module = manifest.module.as_ref().ok_or(PublishError::NoModule)?;
    let manifest_string = toml::to_string(&manifest)?;
    let readme = module.readme.as_ref().and_then(|readme_path| {
        fs::read_to_string(manifest.base_directory_path.join(readme_path)).ok()
    });
    let module_path = manifest.module_path()?;
    let q = PublishPackageMutation::build_query(publish_package_mutation::Variables {
        name: module.name.to_string(),
        version: module.version.clone(),
        description: module.description.clone(),
        manifest: manifest_string,
        license: module.license.clone(),
        readme,
        file_name: Some("module".to_string()),
    });
    let _response: publish_package_mutation::ResponseData =
        execute_query_modifier(&q, |f| f.file("module", &module_path).unwrap())?;
    Ok(())
}

#[derive(Debug, Fail)]
enum PublishError {
    #[fail(display = "Cannot publish without a module.")]
    NoModule,
}

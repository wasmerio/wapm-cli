use crate::graphql::execute_query_modifier;
use crate::manifest::Manifest;

use graphql_client::*;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/publish_package.graphql",
    response_derives = "Debug"
)]
struct PublishPackageMutation;

pub fn publish() -> Result<(), failure::Error> {
    let manifest = Manifest::find_in_current_directory()?;
    let readme = manifest.read_readme_to_string();
    let module_path = manifest.module_path()?;
    let q = PublishPackageMutation::build_query(publish_package_mutation::Variables {
        name: manifest.name.to_string(),
        version: manifest.version,
        description: manifest.description,
        license: manifest.license,
        readme,
        file_name: Some("module".to_string()),
    });
    let _response: publish_package_mutation::ResponseData =
        execute_query_modifier(&q, |f| f.file("module", &module_path).unwrap())?;
    Ok(())
}

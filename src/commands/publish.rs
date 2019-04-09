use crate::graphql::execute_query_modifier;
use crate::manifest::Manifest;

use flate2::{write::GzEncoder, Compression};
use graphql_client::*;
use std::fs;
use std::io::Write;
use tar::Builder;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/publish_package.graphql",
    response_derives = "Debug"
)]
struct PublishPackageMutation;

pub fn publish() -> Result<(), failure::Error> {
    let mut builder = Builder::new(Vec::new());

    let manifest = Manifest::find_in_current_directory()?;
    builder.append_path(manifest.manifest_path()?)?;
    let module = manifest.module.as_ref().ok_or(PublishError::NoModule)?;
    let manifest_string = toml::to_string(&manifest)?;
    let readme = module.readme.as_ref().and_then(|readme_path| {
        if let Err(_) = builder.append_path(readme_path) {
            // Maybe do something here
        }
        fs::read_to_string(manifest.base_directory_path.join(readme_path)).ok()
    });
    let module_path = manifest.module_path()?;
    builder
        .append_path(&module_path)
        .map_err(|_| PublishError::NoModule)?;

    let tar_archive_data = builder.into_inner().map_err(|_|
                                                        // TODO:
                                                        PublishError::NoModule)?;

    //manifest.package.name
    let archive_name = "test.tar.gz".to_string();
    let archive_path = manifest.get_archive_path()?;
    let mut compressed_archive = fs::File::create(archive_path.clone()).unwrap();
    let mut gz_enc = GzEncoder::new(&mut compressed_archive, Compression::default());

    gz_enc.write_all(&tar_archive_data).unwrap();
    let _compressed_archive = gz_enc.finish().unwrap();

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
        execute_query_modifier(&q, |f| f.file(archive_name, archive_path).unwrap())?;
    Ok(())
}

#[derive(Debug, Fail)]
enum PublishError {
    #[fail(display = "Cannot publish without a module.")]
    NoModule,
}

#![cfg_attr(
    not(feature = "full"),
    allow(dead_code, unused_imports, unused_variables)
)]
use crate::graphql::execute_query;
use graphql_client::*;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/get_interface_version.graphql",
    response_derives = "Debug"
)]
struct GetInterfaceVersionQuery;

#[derive(Debug)]
pub struct InterfaceFromServer {
    pub name: String,
    pub version: String,
    pub content: String,
}

impl InterfaceFromServer {
    fn get_response(
        name: String,
        version: String,
    ) -> anyhow::Result<get_interface_version_query::ResponseData> {
        let q = GetInterfaceVersionQuery::build_query(get_interface_version_query::Variables {
            name,
            version,
        });
        execute_query(&q)
    }

    pub fn get(name: String, version: String) -> anyhow::Result<Self> {
        let response = Self::get_response(name, version)?;
        let response_val = response
            .interface
            .ok_or_else(|| anyhow!("Error downloading Interface from the server"))?;
        Ok(Self {
            name: response_val.interface.name,
            version: response_val.version,
            content: response_val.content,
        })
    }
}

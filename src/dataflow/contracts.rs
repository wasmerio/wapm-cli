use crate::graphql::execute_query;
use graphql_client::*;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/get_contract_version.graphql",
    response_derives = "Debug"
)]
struct GetContractVersionQuery;

#[derive(Debug)]
pub struct ContractFromServer {
    pub name: String,
    pub version: String,
    pub content: String,
}

impl ContractFromServer {
    fn get_response(
        name: String,
        version: String,
    ) -> Result<get_contract_version_query::ResponseData, failure::Error> {
        let q = GetContractVersionQuery::build_query(get_contract_version_query::Variables {
            name,
            version,
        });
        execute_query(&q)
    }

    pub fn get(name: String, version: String) -> Result<Self, failure::Error> {
        let response = Self::get_response(name, version)?;
        let response_val = response
            .contract
            .ok_or_else(|| format_err!("Error downloading Contract from the server"))?;
        Ok(Self {
            name: response_val.contract.name,
            version: response_val.version,
            content: response_val.content,
        })
    }
}

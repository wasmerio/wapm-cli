use crate::graphql::execute_query;
use graphql_client::*;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/whoami.graphql",
    response_derives = "Debug"
)]
struct WhoAmIQuery;

pub fn whoami() -> Result<(), failure::Error> {
    let q = WhoAmIQuery::build_query(who_am_i_query::Variables {});
    let response: who_am_i_query::ResponseData = execute_query(&q)?;
    let username = match response.viewer {
        Some(viewer) => viewer.username,
        None => "(not logged in)".to_string(),
    };
    println!("{}", username);
    Ok(())
}

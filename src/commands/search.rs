use crate::graphql::execute_query;

use graphql_client::*;

use prettytable::format;
use prettytable::Table;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct SearchOpt {
    #[structopt(parse(from_str))]
    query: String,
}

type DateTime = String;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/search.graphql",
    response_derives = "Debug"
)]
struct SearchQuery;

pub fn search(options: SearchOpt) -> Result<(), failure::Error> {
    let query = options.query;
    let q = SearchQuery::build_query(search_query::Variables {
        query: query.to_string(),
    });
    let response: search_query::ResponseData = execute_query(&q)?;

    if response.search.edges.len() == 0 {
        println!("No packages found for \"{}\"", query);
        return Ok(());
    }
    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);

    // Add a row per time
    table.add_row(row!["NAME", "DESCRIPTION", "DATE", "VERSION"]);
    for edge in response.search.edges.into_iter() {
        let node = edge.unwrap().node;
        match node {
            Some(search_query::SearchQuerySearchEdgesNode::PackageVersion(version)) => {
                table.add_row(row![
                    version.package.name,
                    version.description,
                    version.created_at[..10],
                    version.version
                ]);
            }
            _ => {}
        }
    }
    table.printstd();

    Ok(())
}

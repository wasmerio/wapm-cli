//! Module for wax, executes a module immediately

use crate::graphql::execute_query;
use crate::data::wax_index;
use crate::dataflow::find_command_result::{self, FindCommandResult};

use graphql_client::*;

use std::ffi::OsString;
use std::env;
use std::convert::From;
use std::str::FromStr;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct ExecuteOpt {
    /// Command name
    command: String,
    /// Run unsandboxed emscripten modules too
    #[structopt(long = "emscripten")]
    run_emscripten_too: bool,
    /// WASI pre-opened directory
    #[structopt(long = "dir", multiple = true, group = "wasi")]
    pre_opened_directories: Vec<String>,
    /// Application arguments
    #[structopt(raw(multiple = "true"), parse(from_os_str))]
    args: Vec<OsString>,
}

#[derive(Debug, Fail)]
enum ExecuteError {
    #[fail(display = "No package for command `{}` found locally or in the registry", name)]
    CommandNotFound { name: String },
    #[fail(display = "The command `{}` is using the Emscripten ABI which may be implmented in a way that is partially unsandbooxed.  To opt-in to executing Emscripten Wasm modules run the command again with the `--emscripten` flag", name)]
    EmscriptenDisabled { name: String },
    #[fail(display = "Error with the Wax index: {}", _0)]
    WaxIndexError(wax_index::WaxIndexError),
    #[fail(display = "Error parsing data from register: {}", _0)]
    ErrorInDataFromRegistry(String),
}


#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/wax_get_command.graphql",
    response_derives = "Debug"
)]
struct WaxGetCommandQuery;

pub fn execute(opt: ExecuteOpt) -> Result<(), failure::Error> {
    let command_name = opt.command.as_str();
    let args = &opt.args;
    let current_dir = env::current_dir()?;   

    // first search locally for the command

    // if not found, query the server and check if we already have it installed
    let q = WaxGetCommandQuery::build_query(wax_get_command_query::Variables {
        command: command_name.to_string(),
    });
    let response: wax_get_command_query::ResponseData = execute_query(&q)?;
    if let Some(command) = response.command {
        // command found, check if it's installed
        if let Some(abi) = command.module.abi.as_ref() {
            if abi == "emscripten" && !opt.run_emscripten_too {
                return Err(ExecuteError::EmscriptenDisabled { name: command_name.to_string() }.into());
            }
        }
        let mut wax_index = wax_index::WaxIndex::open()?;

        if let Ok(wax_index::WaxIndexEntry {
            version,
            location,
        }) = wax_index.search_for_entry(command_name.to_string()) {
            let found_version = semver::Version::from_str(&command.package_version.version).map_err(|e| ExecuteError::ErrorInDataFromRegistry(e.to_string()))?;
            if found_version > version {
                // install newer version
                // TODO: create API for inserting/installing more easily
            } else {
                // use existing version
            }
        }

        wax_index.save();
        
        dbg!(command);
    } else {
        return Err(ExecuteError::CommandNotFound { name: command_name.to_string() }.into());
    }

    Ok(())
}

fn run(command_name: String, location: PathBuf) -> Result<(), failure::Error> {
    let find_command_result::Command {
        source: source_path_buf,
        manifest_dir,
        args: _,
        module_name,
        is_global,
        prehashed_cache_key,
    } = match FindCommandResult::find_command_in_directory(&location, &command_name) {
        FindCommandResult::CommandNotFound(s) => {
            
        },
        FindCommandResult::CommandFound {
            source,
            manifest_dir,
            args,
            module_name,
            prehashed_cache_key,
        } => {
            unimplemented!()
        }
        FindCommandResult::Error(e) => return Err(e),
    };

    // TODO: refactor and reuse pieces of command/run

    debug!(
        "Running module located at {:?}",
        &run_dir.join(&source_path_buf)
    );

    Ok(())
}

impl From<wax_index::WaxIndexError> for ExecuteError {
    fn from(other: wax_index::WaxIndexError) -> Self {
        ExecuteError::WaxIndexError(other)
    }
}

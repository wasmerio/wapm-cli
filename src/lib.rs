#[macro_use]
extern crate log;
#[cfg(feature = "package")]
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate serde_derive;
#[cfg(feature = "prettytable-rs")]
#[macro_use]
extern crate prettytable;

#[cfg(test)]
#[macro_use]
extern crate toml;

#[cfg(feature = "integration_tests")]
pub mod integration_tests;

mod abi;
pub mod commands;
mod config;
mod constants;
pub mod data;
#[cfg(feature = "full")]
mod database;
mod dataflow;
mod graphql;
mod init;
#[cfg(feature = "full")]
mod interfaces;
mod keys;
pub mod logging;
#[cfg(not(target_os = "wasi"))]
mod proxy;
mod sql;
#[cfg(feature = "update-notifications")]
pub mod update_notifier;
pub mod util;
mod validate;
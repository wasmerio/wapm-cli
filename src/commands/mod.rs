//! List of exported subcommands for use by wapm

mod add;
mod bin;
mod completions;
mod config;
mod execute;
mod init;
mod install;
#[cfg(feature = "full")]
mod keys;
#[cfg(feature = "full")]
mod list;
mod login;
mod logout;
#[cfg(feature = "full")]
mod publish;
mod remove;
mod run;
#[cfg(feature = "full")]
mod search;
mod uninstall;
mod validate;
mod whoami;

pub use self::add::{add, AddOpt};
pub use self::bin::{bin, BinOpt};
pub use self::completions::CompletionOpt;
pub use self::config::{config, ConfigOpt};
pub use self::execute::{execute, ExecuteOpt};
pub use self::init::{init, InitOpt};
pub use self::install::{install, InstallOpt};
#[cfg(feature = "full")]
pub use self::keys::{keys, KeyOpt};
#[cfg(feature = "full")]
pub use self::list::{list, ListOpt};
pub use self::login::login;
pub use self::logout::logout;
#[cfg(feature = "full")]
pub use self::publish::{publish, PublishOpt};
pub use self::remove::{remove, RemoveOpt};
pub use self::run::{run, RunOpt};
#[cfg(feature = "full")]
pub use self::search::{search, SearchOpt};
pub use self::uninstall::{uninstall, UninstallOpt};
pub use self::validate::{validate, ValidateOpt};
pub use self::whoami::whoami;

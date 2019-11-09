//! Code pertaining to the `remove` subcommand: it removes dependencies
//! from the manifest.

use crate::data::manifest::Manifest;
use structopt::StructOpt;

/// Options for the `remove` subcommand
#[derive(StructOpt, Debug)]
pub struct RemoveOpt {
    packages: Vec<String>,
}

#[derive(Debug, Fail)]
enum RemoveError {
    #[fail(display = "There were problems removing packages")]
    GenericError,
    #[fail(display = "No packages to remove; could not find a manifest in the current directory")]
    NoManifest,
    #[fail(display = "No packages listed to remove")]
    ArgumentsRequired,
}

/// Run the remove command
pub fn remove(options: RemoveOpt) -> Result<(), failure::Error> {
    let mut error = false;
    let mut manifest: Manifest = {
        let cur_dir = std::env::current_dir()?;
        Manifest::find_in_directory(cur_dir).map_err(|_| RemoveError::NoManifest)?
    };

    if options.packages.is_empty() {
        return Err(RemoveError::ArgumentsRequired.into());
    }

    for package_name in options.packages {
        if package_name.contains('@') {
            error = true;
            error!(
                "`wapm remove` can not remove specific versions. Try to remove \"{}\" again without the version",
                package_name
            );
            continue;
        }

        if manifest.remove_dependency(&package_name).is_some() {
            info!("Removing \"{}\"", &package_name);
        }
    }

    manifest.save()?;

    if error {
        Err(RemoveError::GenericError.into())
    } else {
        println!("Packages successfully removed!");
        Ok(())
    }
}

#[cfg(feature = "integration_tests")]
impl RemoveOpt {
    pub fn new(packages: Vec<String>) -> Self {
        RemoveOpt { packages }
    }
}

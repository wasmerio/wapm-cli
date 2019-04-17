//! logic to init a directory for use with wapm

use crate::manifest::MANIFEST_FILE_NAME;
use crate::util;
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
};

fn construct_template_manifest_from_data(username: Option<String>, package_name: String) -> String {
    let name_string = if let Some(un) = username {
        format!("{}/{}", un, package_name)
    } else {
        package_name
    };
    format!(
        r#"[package]
name = "{}"
version = "0.1.0"
description = ""
"#,
        name_string
    )
}

pub fn init(dir: PathBuf, package_name: String) -> Result<(), failure::Error> {
    init_manifest(dir.clone(), package_name)?;
    #[allow(unused_must_use)]
    {
        init_gitignore(dir);
    }
    Ok(())
}

pub fn init_manifest(dir: PathBuf, package_name: String) -> Result<(), failure::Error> {
    let manifest = {
        let mut dir = dir.clone();
        dir.push(MANIFEST_FILE_NAME);
        dir
    };
    if manifest.exists() {
        return Err(InitError::ManifestAlreadyExists { dir }.into());
    }

    let mut f = fs::File::create(manifest)?;
    f.write(construct_template_manifest_from_data(util::get_username()?, package_name).as_bytes())?;

    Ok(())
}

pub fn init_gitignore(mut dir: PathBuf) -> Result<(), failure::Error> {
    let gitignore = {
        dir.push(".gitignore");
        dir
    };

    let mut f = fs::OpenOptions::new()
        .create(false)
        .read(true)
        .append(true)
        .open(gitignore)?;
    let mut gitignore_str = String::new();
    f.read_to_string(&mut gitignore_str)?;

    // TODO: this doesn't understand gitignores at all, it just checks for an entry
    // use crate that can check if a directory is ignored or not
    for line in gitignore_str.lines() {
        if line.contains("wapm_packages") {
            return Ok(());
        }
    }

    f.write(b"\nwapm_packages")?;
    Ok(())
}

#[derive(Debug, Fail)]
pub enum InitError {
    #[fail(display = "Manifest file already exists in {:?}", dir)]
    ManifestAlreadyExists { dir: PathBuf },
}

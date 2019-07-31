//! This module is for updating the user about the latest version of Wapm/Wasmer
//!
//! This is turned on in our releases by default but is off when building from source

use crate::{config, proxy, util};
use colored::*;
use reqwest::{
    header::{HeaderValue, ACCEPT},
    Client, RedirectPolicy, Response,
};
use std::fmt::Write;

const GITHUB_RELEASE_PAGE: &str = "https://github.com/wasmerio/wasmer/releases/latest";
const GITHUB_RELEASE_URL_BASE: &str = "https://github.com/wasmerio/wasmer/releases/tag/";

#[derive(Debug, Deserialize)]
struct VersionResponse {
    tag_name: String,
}

/// this is the base call, it will spawn another process
pub fn run_async_check_base() -> Option<()> {
    if let Some((last_checked_time, maybe_next_version)) =
        config::Config::update_notifications_enabled()
            .and_then(|()| config::get_last_update_checked_time())
    {
        // if we have it cached, then call check to pull it out of the cache and print it
        if let Some(message) = maybe_next_version
            .and_then(|next_version| check(Some(next_version)))
            .and_then(|res| format_message(&res.old_version, &res.new_version, &res.release_url))
        {
            print_message(&message);
            // clear the cache
            config::set_last_update_checked_time(None);
        } else {
            // otherwise, if it's time to check again, spawn a background process to update the cache
            let now = time::now();
            let time_to_check: time::Duration =
                time::Duration::from_std(std::time::Duration::from_secs(60 * 60 * 24)).unwrap();
            if now - last_checked_time >= time_to_check {
                config::lock_background_process()?;
                // lock and check for lock
                std::process::Command::new("wapm")
                    .arg("run-background-update-check")
                    .spawn()
                    .ok()?;
            }
        }
    }

    None
}

/// this is the check run by the process spawned by `run_async_check_base`
pub fn run_subprocess_check() {
    if let None = run_subprocess_check_inner() {
        debug!("Background check failed");
    }
    config::unlock_background_process();
}

fn run_subprocess_check_inner() -> Option<()> {
    check(None).and_then(|res| config::set_last_update_checked_time(Some(&res.new_version)))?;
    Some(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueryResult {
    pub old_version: String,
    pub new_version: String,
    pub release_url: String,
}

pub fn check(maybe_next_version: Option<String>) -> Option<QueryResult> {
    let version_tag = if let Some(v_tag) = maybe_next_version {
        v_tag
    } else {
        let builder = Client::builder();
        let client = match proxy::maybe_set_up_proxy() {
            Ok(Some(proxy)) => builder.proxy(proxy),
            Ok(None) => builder, //continue without proxy
            Err(_) => return None,
        }
        .redirect(RedirectPolicy::limited(10))
        .build()
        .ok()?;

        let mut response: Response = client
            .get(GITHUB_RELEASE_PAGE)
            .header(ACCEPT, HeaderValue::from_static("application/json"))
            .send()
            .ok()?;
        let response_content: VersionResponse = response.json().ok()?;
        response_content.tag_name
    };

    if version_tag.is_empty() {
        return None;
    }

    let installed_wasmer_version = util::get_latest_runtime_version()?;

    if installed_wasmer_version != version_tag {
        Some(QueryResult {
            old_version: installed_wasmer_version,
            new_version: version_tag.to_string(),
            release_url: format!("{}{}", GITHUB_RELEASE_URL_BASE, version_tag),
        })
    } else {
        None
    }
}

const HORIZONTAL_LINE_CHAR: &str = "─";
const TOP_LEFT_LINE_CHAR: &str = "╭";
const TOP_RIGHT_LINE_CHAR: &str = "╮";
const MID_LINE_CHAR: &str = "│";
const BOT_LEFT_LINE_CHAR: &str = "╰";
const BOT_RIGHT_LINE_CHAR: &str = "╯";
const PAD_AMOUNT: usize = 2;

fn prefix_line(out: &mut String) -> Result<(), std::fmt::Error> {
    for _ in 0..4 {
        out.write_char(' ')?;
    }
    Ok(())
}

// assumes left, mid, and right are 1 character long
fn write_solid_line(
    out: &mut String,
    max_line_len: usize,
    left: &str,
    mid: &str,
    right: &str,
) -> Result<(), std::fmt::Error> {
    prefix_line(out)?;
    out.write_str(&left.yellow())?;
    for _ in 0..(max_line_len + PAD_AMOUNT * 2) {
        out.write_str(&mid.yellow())?;
    }
    out.write_str(&right.yellow())?;
    out.write_char('\n')?;
    Ok(())
}

fn write_mid_line(
    out: &mut String,
    max_line_len: usize,
    line_to_write: &str,
    line_len: usize,
) -> Result<(), std::fmt::Error> {
    let size_delta = max_line_len - line_len;
    let offset_amount = size_delta / 2;
    prefix_line(out)?;
    out.write_str(&MID_LINE_CHAR.yellow())?;
    for _ in 0..offset_amount + PAD_AMOUNT {
        out.write_char(' ')?;
    }
    out.write_str(&line_to_write)?;
    for _ in 0..(size_delta - offset_amount) + PAD_AMOUNT {
        out.write_char(' ')?;
    }
    out.write_str(&MID_LINE_CHAR.yellow())?;
    out.write_char('\n')?;
    Ok(())
}

fn format_message(
    old_version_str: &str,
    new_version_str: &str,
    changelog_url: &str,
) -> Option<String> {
    let hook_prefix = "There's a new version of wasmer and wapm! ";
    let hook_prefix_len = hook_prefix.chars().count();
    let rest_of_hook_len = old_version_str.chars().count() + 3 + new_version_str.chars().count();
    let hook_len = hook_prefix_len + rest_of_hook_len;

    let changelog_prefix = "Changelog: ";
    let changelog_prefix_len = changelog_prefix.chars().count();
    let changelog_len = changelog_prefix_len + changelog_url.chars().count();

    let cta_prefix = "Update with ";
    let update_command = "wasmer self-update";
    let cta = format!("{}{}!", cta_prefix, update_command.green().bold());
    let cta_len = cta_prefix.chars().count() + update_command.chars().count() + 1;

    let max_line_len = std::cmp::max(std::cmp::max(hook_len, changelog_len), cta_len);

    let mut out = String::new();

    write_solid_line(
        &mut out,
        max_line_len,
        TOP_LEFT_LINE_CHAR,
        HORIZONTAL_LINE_CHAR,
        TOP_RIGHT_LINE_CHAR,
    )
    .ok()?;
    let hook_str = format!(
        "{}{} → {}",
        hook_prefix,
        old_version_str.red(),
        new_version_str.green()
    );
    write_mid_line(&mut out, max_line_len, &hook_str, hook_len).ok()?;
    let cl_str = format!("{}{}", changelog_prefix, changelog_url);
    write_mid_line(&mut out, max_line_len, &cl_str, changelog_len).ok()?;
    write_mid_line(&mut out, max_line_len, &cta, cta_len).ok()?;

    write_solid_line(
        &mut out,
        max_line_len,
        BOT_LEFT_LINE_CHAR,
        HORIZONTAL_LINE_CHAR,
        BOT_RIGHT_LINE_CHAR,
    )
    .ok()?;

    Some(out)
}

pub fn print_message(release_str: &str) {
    println!("{}", release_str)
}

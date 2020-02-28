//! This module is for updating the user about the latest version of Wapm/Wasmer
//!
//! This is turned on in our releases by default but is off when building from source

use crate::{config, proxy, util};
use chrono::{DateTime, Utc};
use colored::*;
use reqwest::{
    header::{HeaderValue, ACCEPT},
    Client, RedirectPolicy, Response,
};
use std::fmt::Write;
use std::fs::File;
use std::path::PathBuf;

const GITHUB_RELEASE_PAGE: &str = "https://github.com/wasmerio/wasmer/releases/latest";
const GITHUB_RELEASE_URL_BASE: &str = "https://github.com/wasmerio/wasmer/releases/tag/";
const GLOBAL_WAPM_UPDATE_FILE: &str = ".wapm_update.json";
const BACKGROUND_UPDATE_CHECK_RUNNING: &str = ".background_update_process_running.txt";

/// The amount of seconds that we need to wait until showing the next notification
const CHECK_DURATION_IN_SECONDS: u64 = 60 * 60 * 24; // 24 hours
const WAPM_NOTIFICATION_WINDOW: u64 = 60 * 60 * 2; // 2 hours

#[derive(Debug, Deserialize)]
struct VersionResponse {
    tag_name: String,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Default)]
pub struct WapmUpdate {
    /// The data related to the last check on the Github Registry
    pub last_check: Option<WapmLastCheck>,
    /// The time when wapm last trigger the notification
    pub last_notified: Option<DateTime<Utc>>,
}

impl WapmUpdate {
    fn load() -> Result<Self, String> {
        let path = get_wapm_update_file_path();
        let json_file = File::open(path).map_err(|err| err.to_string())?;
        let wasm_update: WapmUpdate =
            serde_json::from_reader(json_file).map_err(|err| err.to_string())?;
        Ok(wasm_update)
    }
    fn load_or_default() -> Self {
        Self::load().unwrap_or_default()
    }
    fn save(&self) -> Result<(), String> {
        let path = get_wapm_update_file_path();
        let json_file = File::create(path).map_err(|err| err.to_string())?;
        serde_json::to_writer(json_file, &self).map_err(|err| err.to_string())?;
        Ok(())
    }
    fn set_last_check(&mut self, version: String) {
        let now = Utc::now();
        self.last_check = Some(WapmLastCheck {
            timestamp: now,
            version: version,
        });
    }
    fn should_trigger_check(&self) -> bool {
        match self.last_check {
            Some(ref last_check) => {
                let now = Utc::now();
                let time_to_check: time::Duration = time::Duration::from_std(
                    std::time::Duration::from_secs(CHECK_DURATION_IN_SECONDS),
                )
                .unwrap();
                now - last_check.timestamp >= time_to_check
            }
            None => true,
        }
    }
    fn maybe_print_notification(&mut self) -> Result<(), String> {
        let last_check = self.last_check.as_ref();
        match last_check {
            None => Ok(()),
            Some(last_check) => {
                let now = Utc::now();
                if let Some(last_notified) = self.last_notified {
                    let time_to_check: time::Duration = time::Duration::from_std(
                        std::time::Duration::from_secs(WAPM_NOTIFICATION_WINDOW),
                    )
                    .unwrap();
                    if now - last_notified < time_to_check {
                        return Ok(());
                    }
                }

                let new_version = last_check.version.to_owned();
                // We use wasmer and not constants::DEFAULT_RUNTIME because the
                // update logic is very tied to wasmer itself.
                let old_version = util::get_latest_runtime_version("wasmer")?;

                if let Some(b) = util::compare_versions(&old_version, &new_version) {
                    if b {
                        return Ok(());
                    }
                } else {
                    // fall back to direct comparison
                    // If we are in the same version
                    if old_version == new_version {
                        return Ok(());
                    }
                }

                let release_url = format!("{}{}", GITHUB_RELEASE_URL_BASE, new_version);
                let message = format_message(&old_version, &new_version, &release_url).unwrap();
                print!("\n{}", message);
                self.last_notified = Some(now);
                self.save()
            }
        }
    }
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct WapmLastCheck {
    pub timestamp: DateTime<Utc>,
    pub version: String,
}

fn get_wapm_update_file_path() -> PathBuf {
    let mut path = config::Config::get_folder().unwrap();
    path.push(GLOBAL_WAPM_UPDATE_FILE);
    path
}

/// this is the base call, it will spawn another process
pub fn run_async_check_base() {
    if !config::Config::update_notifications_enabled() {
        return;
    }
    let wapm_update = WapmUpdate::load_or_default();
    if wapm_update.should_trigger_check() {
        // lock and check for lock
        if !lock_background_process() {
            return;
        }

        let current_wapm = std::env::current_exe().expect("Can't get current wapm executable");
        let current_dir = std::env::current_dir().expect("Can't get current wapm dir");
        std::process::Command::new(current_wapm)
            .arg("run-background-update-check")
            .current_dir(current_dir)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("Can't spawn the background update check");
    }
}

pub fn check_sync() {
    if !config::Config::update_notifications_enabled() {
        return;
    }
    match WapmUpdate::load() {
        Ok(mut wapm_update) => {
            wapm_update
                .maybe_print_notification()
                .expect("Can't show wapm update notification");
        }
        Err(_) => {}
    }
}

/// this is the check run by the process spawned by `run_async_check_base`
pub fn run_subprocess_check() {
    match get_latest_tag() {
        Ok(new_version) => {
            let mut wapm_update = WapmUpdate::load_or_default();
            wapm_update.set_last_check(new_version);
            wapm_update.last_notified = None;
            wapm_update.save().expect("Save to file failed");
        }
        Err(e) => {
            error!("Background check failed: {}", e);
        }
    }
    try_unlock_background_process()
}

pub fn get_latest_tag() -> Result<String, String> {
    let builder = Client::builder();
    let client = match proxy::maybe_set_up_proxy() {
        Ok(Some(proxy)) => builder.proxy(proxy),
        Ok(None) => builder, //continue without proxy
        Err(e) => return Err(e.to_string()),
    }
    .redirect(RedirectPolicy::limited(10))
    .build()
    .map_err(|err| err.to_string())?;

    let mut response: Response = client
        .get(GITHUB_RELEASE_PAGE)
        .header(ACCEPT, HeaderValue::from_static("application/json"))
        .send()
        .map_err(|err| err.to_string())?;

    let response_content: VersionResponse = response.json().map_err(|err| err.to_string())?;
    Ok(response_content.tag_name)
}

/// Atomically check if a file exists and create it if it doesn't
/// this function is used in the background updater to prevent wapm from
/// spawning a ton of background processes and acting like a fork bomb
pub fn lock_background_process() -> bool {
    let mut path = match config::Config::get_folder() {
        Ok(folder) => folder,
        _ => return false,
    };
    path.push(BACKGROUND_UPDATE_CHECK_RUNNING);
    let file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path);
    return file.is_ok();
}

pub fn try_unlock_background_process() {
    let mut path = match config::Config::get_folder() {
        Ok(folder) => folder,
        _ => return,
    };
    path.push(BACKGROUND_UPDATE_CHECK_RUNNING);
    if let Err(e) = std::fs::remove_file(&path) {
        debug!(
            "File {} was deleted while running background check or unlock was called without lock being called first: {:?}",
            path.to_string_lossy(), e
        )
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
) -> Result<String, std::fmt::Error> {
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
    )?;
    let hook_str = format!(
        "{}{} → {}",
        hook_prefix,
        old_version_str.red(),
        new_version_str.green()
    );
    write_mid_line(&mut out, max_line_len, &hook_str, hook_len)?;
    let cl_str = format!("{}{}", changelog_prefix, changelog_url);
    write_mid_line(&mut out, max_line_len, &cl_str, changelog_len)?;
    write_mid_line(&mut out, max_line_len, &cta, cta_len)?;

    write_solid_line(
        &mut out,
        max_line_len,
        BOT_LEFT_LINE_CHAR,
        HORIZONTAL_LINE_CHAR,
        BOT_RIGHT_LINE_CHAR,
    )?;

    Ok(out)
}

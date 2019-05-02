//! Utilities for setting up logging

use crate::config::Config;
use crate::util;
use fern::colors::{Color, ColoredLevelConfig};
use std::{fs, path::PathBuf};

/// Subroutine to instantiate the loggers
pub fn set_up_logging() -> Result<(), failure::Error> {
    let colors_line = ColoredLevelConfig::new()
        .error(Color::Red)
        .warn(Color::Yellow)
        .trace(Color::BrightBlack);
    let should_color = util::wapm_should_print_color();

    let colors_level = colors_line.info(Color::Green);
    let dispatch = fern::Dispatch::new()
        // stdout and stderr logging
        .level(log::LevelFilter::Debug)
        .chain({
            let base = if should_color {
                fern::Dispatch::new().format(move |out, message, record| {
                    out.finish(format_args!(
                        "{color_line}[{level}{color_line}]{ansi_close} {message}",
                        color_line = format_args!(
                            "\x1B[{}m",
                            colors_line.get_color(&record.level()).to_fg_str()
                        ),
                        level = colors_level.color(record.level()),
                        ansi_close = "\x1B[0m",
                        message = message,
                    ));
                })
            } else {
                // default formatter without color
                fern::Dispatch::new().format(move |out, message, record| {
                    out.finish(format_args!(
                        "[{level}] {message}",
                        level = record.level(),
                        message = message,
                    ));
                })
            };
            base
                // stdout
                .chain(
                    fern::Dispatch::new()
                        .filter(|metadata| {
                            metadata.level() == log::LevelFilter::Info
                                && metadata.target().starts_with("wapm_cli")
                        })
                        .chain(std::io::stdout()),
                )
                // stderr
                .chain(
                    fern::Dispatch::new()
                        .filter(|metadata| {
                            // lower is higher priority
                            metadata.level() <= log::LevelFilter::Warn
                                && metadata.target().starts_with("wapm_cli")
                        })
                        .chain(std::io::stderr()),
                )
        });

    // verbose logging to file
    let dispatch = if let Ok(wasmer_dir) = Config::get_folder() {
        let mut log_out = PathBuf::from(wasmer_dir);
        log_out.push("wapm.log");
        dispatch.chain(
            fern::Dispatch::new()
                .format(move |out, message, record| {
                    out.finish(format_args!(
                        "[{date}][{level}][{target}][{file}:{line}] {message}",
                        date = chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                        target = record.target(),
                        level = record.level(),
                        message = message,
                        file = record.file().unwrap_or(""),
                        line = record
                            .line()
                            .map(|line| format!("{}", line))
                            .unwrap_or("".to_string()),
                    ));
                })
                .level(log::LevelFilter::Debug)
                .level_for("hyper", log::LevelFilter::Info)
                .level_for("tokio_reactor", log::LevelFilter::Info)
                .chain(
                    fs::OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .create(true)
                        .open(log_out)
                        .map_err(|e| {
                            LoggingError::FailedToOpenLoggingFile(format!(
                                "error type: {:?}",
                                e.kind()
                            ))
                        })?,
                ),
        )
    } else {
        dispatch
    };

    dispatch
        .apply()
        .map_err(|e| LoggingError::FailedToInstantiateLogger(format!("{}", e)))?;

    trace!("Logging set up");
    Ok(())
}

#[derive(Debug, Fail)]
pub enum LoggingError {
    #[fail(display = "Failed to open logging file in WASMER_DIR: {}", _0)]
    FailedToOpenLoggingFile(String),
    #[fail(display = "Something went wrong setting up logging: {}", _0)]
    FailedToInstantiateLogger(String),
}

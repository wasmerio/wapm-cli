//! Utilities for setting up logging

use crate::config::Config;
use crate::util;
use fern::colors::{Color, ColoredLevelConfig};
use std::fs;
use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};

static STDOUT_LINE_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn get_num_stdout_lines_logged() -> usize {
    STDOUT_LINE_COUNTER.load(Ordering::Acquire)
}

/// Updates counter with the lines printed to stdout
pub(crate) fn add_lines_printed_to_stdout(num_lines: usize) {
    STDOUT_LINE_COUNTER.fetch_add(num_lines, Ordering::AcqRel);
}

/// Note: this function doesn't lock the atomic, so using it from
/// multiple threads may not work.
pub(crate) fn clear_stdout() -> io::Result<()> {
    use std::io::Write;

    let stdout = io::stdout();
    let mut f = stdout.lock();
    let num_lines_to_clear = get_num_stdout_lines_logged();
    for _ in 0..num_lines_to_clear {
        // ANSI escape codes for:
        // - go up one line: \x1B[<NUM LINES>A
        // - clear the line: \x1B[2K
        write!(f, "\x1B[1A\x1B[2K")?;
    }
    f.flush()?;
    Ok(())
}

/// Subroutine to instantiate the loggers
pub fn set_up_logging(count_lines: bool) -> Result<(), failure::Error> {
    let colors_line = ColoredLevelConfig::new()
        .error(Color::Red)
        .warn(Color::Yellow)
        .trace(Color::BrightBlack);
    let should_color = util::wapm_should_print_color();

    let colors_level = colors_line.info(Color::Green);
    let dispatch = fern::Dispatch::new()
        // stdout and stderr logging
        .level(log::LevelFilter::Info)
        .filter(|metadata| metadata.target().starts_with("wapm_cli"))
        .chain({
            let base = if should_color {
                fern::Dispatch::new().format(move |out, message, record| {
                    if count_lines && record.level() == log::Level::Info {
                        let num_lines = message.to_string().lines().count();
                        add_lines_printed_to_stdout(num_lines);
                    }
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
                    if count_lines && record.level() == log::Level::Info {
                        let num_lines = message.to_string().lines().count();
                        add_lines_printed_to_stdout(num_lines);
                    }
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
                        .filter(|metadata| metadata.level() == log::LevelFilter::Info)
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
        let log_out = wasmer_dir.join("wapm.log");
        dispatch.chain(
            fern::Dispatch::new()
                .level(log::LevelFilter::Debug)
                .level_for("hyper", log::LevelFilter::Info)
                .level_for("tokio_reactor", log::LevelFilter::Info)
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
                            .unwrap_or_else(|| "".to_string()),
                    ));
                })
                .chain(
                    fs::OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(true)
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

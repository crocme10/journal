use crate::watcher;
use snafu::{Backtrace, Snafu};
use std::{io, num};
use tokio::sync::mpsc::error::SendError;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("User Error: {}", details))]
    #[snafu(visibility(pub))]
    UserError { details: String },

    #[snafu(display("Env Error: {}", details))]
    #[snafu(visibility(pub))]
    EnvError { details: String },

    #[snafu(display("IO Error: {}", source))]
    #[snafu(visibility(pub))]
    IOError {
        source: io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("Parse Error: {}", source))]
    #[snafu(visibility(pub))]
    ParseError {
        source: num::ParseIntError,
        backtrace: Backtrace,
    },

    #[snafu(display("WatcherError: {} / {}", source, backtrace))]
    #[snafu(visibility(pub))]
    WatcherError {
        source: watcher::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("DB Connection Error: {}", source))]
    #[snafu(visibility(pub))]
    DBConnError {
        source: sqlx::error::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("MPSC Channel Error: {}", source))]
    #[snafu(visibility(pub))]
    ChannelError {
        source: SendError<String>,
        backtrace: Backtrace,
    },
}

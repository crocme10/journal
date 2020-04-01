use crate::watcher;
use snafu::{Backtrace, Snafu};
use std::io;
use tokio::sync::mpsc::error::SendError;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("User Error: {}", details))]
    #[snafu(visibility(pub))]
    UserError {
        details: String,
        // backtrace: Backtrace,
    },

    #[snafu(display("IO Error: {}", source))]
    #[snafu(visibility(pub))]
    IOError {
        source: io::Error,
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
        source: tokio_postgres::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("DB Pool Error: {}", source))]
    #[snafu(visibility(pub))]
    DBPoolError {
        source: bb8_postgres::bb8::RunError<tokio_postgres::Error>,
        backtrace: Backtrace,
    },

    #[snafu(display("DB Error: {}", source))]
    #[snafu(visibility(pub))]
    DBError {
        source: tokio_postgres::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("MPSC Channel Error: {}", source))]
    #[snafu(visibility(pub))]
    ChannelError {
        source: SendError<String>,
        backtrace: Backtrace,
    },
}

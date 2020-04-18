use super::error;
use snafu::{NoneError, ResultExt};
use std::path::PathBuf;

pub struct Config {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
    pub port: u16,
    pub assets_path: PathBuf,
    pub static_path: PathBuf,
}

impl Config {
    pub fn new() -> Result<Config, error::Error> {
        let cert_path = dotenv::var("CERT_PATH")
            .or(Err(NoneError))
            .context(error::EnvError {
                details: String::from("CERT_PATH"),
            })?;
        let key_path = dotenv::var("KEY_PATH")
            .or(Err(NoneError))
            .context(error::EnvError {
                details: String::from("KEY_PATH"),
            })?;
        let static_path =
            dotenv::var("STATIC_PATH")
                .or(Err(NoneError))
                .context(error::EnvError {
                    details: String::from("STATIC_PATH"),
                })?;
        let assets_path =
            dotenv::var("ASSETS_PATH")
                .or(Err(NoneError))
                .context(error::EnvError {
                    details: String::from("ASSETS_PATH"),
                })?;
        let port = dotenv::var("SERVER_PORT")
            .or(Err(NoneError))
            .context(error::EnvError {
                details: String::from("SERVER_PORT"),
            })?
            .parse::<u16>()
            .context(error::ParseError)?;

        Ok(Config {
            cert_path: PathBuf::from(cert_path),
            key_path: PathBuf::from(key_path),
            port,
            assets_path: PathBuf::from(assets_path),
            static_path: PathBuf::from(static_path),
        })
    }
}

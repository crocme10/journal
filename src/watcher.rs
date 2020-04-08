use crate::model::{Doc, Front};
use chrono::prelude::*;
use futures::future;
use futures::stream::{TryStream, TryStreamExt};
use inotify::{Event, EventMask, Inotify, WatchMask};
use log::debug;
use snafu::{futures::try_stream::TryStreamExt as SnafuTSE, Backtrace, ResultExt, Snafu};
use std::io::{self, BufReader, Read};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("INotify Error: {}", source))]
    #[snafu(visibility(pub))]
    INotifyError {
        source: std::io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("File Error '{}': {}", path.display(), source))]
    #[snafu(visibility(pub))]
    FileIOError {
        path: PathBuf,
        source: std::io::Error,
        backtrace: Backtrace,
    },

    #[snafu(display("File Error: {}", details))]
    #[snafu(visibility(pub))]
    FileError { details: String },

    #[snafu(display("UUID Error: {}", source))]
    #[snafu(visibility(pub))]
    UuidError { source: uuid::Error },

    #[snafu(display("YAML Error: {}", source))]
    #[snafu(visibility(pub))]
    YamlError { source: serde_yaml::Error },
}

pub struct Watcher {
    path: PathBuf,
    buffer: [u8; 4096],
}

impl Watcher {
    pub fn new(path: PathBuf) -> Self {
        Watcher {
            path,
            buffer: [0u8; 4096],
        }
    }

    pub fn doc_stream(
        &mut self,
    ) -> Result<impl TryStream<Ok = Doc, Error = Error> + '_, io::Error> {
        let mut inotify = Inotify::init()?;

        inotify.add_watch(
            self.path.clone(),
            WatchMask::MODIFY | WatchMask::CREATE | WatchMask::DELETE,
        )?;

        let event_stream = inotify.event_stream(&mut self.buffer[..])?;

        Ok(event_stream
            .context(INotifyError)
            .and_then(event_to_path)
            .try_filter_map(|opt_path| future::ok(opt_path))
            .and_then(path_to_doc)
            .try_filter_map(|opt_doc| future::ok(opt_doc)))
    }
}

fn event_to_path(
    event: Event<std::ffi::OsString>,
) -> impl future::TryFuture<Ok = Option<PathBuf>, Error = Error> {
    let opt_path = match event.name {
        Some(name) => {
            let path = PathBuf::from(name.clone());
            if let Some(ext) = path.extension() {
                if ext == "md" {
                    if event.mask.contains(EventMask::CREATE) {
                        if event.mask.contains(EventMask::ISDIR) {
                            // println!("Directory created: {:?}", name);
                            None
                        } else {
                            debug!("File created: {}", path.display());
                            Some(path)
                        }
                    } else if event.mask.contains(EventMask::DELETE) {
                        if event.mask.contains(EventMask::ISDIR) {
                            // println!("Directory deleted: {:?}", path);
                            None
                        } else {
                            debug!("File deleted: {}", path.display());
                            None
                        }
                    } else if event.mask.contains(EventMask::MODIFY) {
                        if event.mask.contains(EventMask::ISDIR) {
                            // println!("Directory modified: {:?}", path);
                            None
                        } else {
                            debug!("File modified: {}", path.display());
                            Some(path)
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        }
        None => None,
    };
    future::ok(opt_path)
}

fn path_to_doc(path: PathBuf) -> impl future::TryFuture<Ok = Option<Doc>, Error = Error> {
    // FIXME: Hardcoded base dir
    let mut p = PathBuf::from("assets");
    p.push(path.clone());

    debug!("Checking file: {}", p.display());
    let res = std::fs::File::open(p.clone())
        .context(FileIOError {
            path: PathBuf::from(p.clone()),
        })
        .and_then(|file| {
            let mut reader = BufReader::new(file);
            let mut buffer = String::new();
            reader.read_to_string(&mut buffer).context(FileIOError {
                path: PathBuf::from(p),
            })?;
            Ok(buffer)
        })
        .and_then(|contents| {
            // This condition occurs when working with eg neovim...
            // I'm not trying to investigate, just guarding against it.
            if contents.len() == 0 {
                return Ok(None);
            }
            let v: Vec<&str> = contents.splitn(3, "---").collect();
            if v.len() < 3 {
                return Err(Error::FileError {
                    details: format!("content length: {}", contents.len()),
                });
            }
            let base = path
                .file_stem()
                .ok_or(snafu::NoneError)
                .context(FileError {
                    details: String::from("Invalid Stem"),
                })?
                .to_str()
                .ok_or(snafu::NoneError)
                .context(FileError {
                    details: String::from("Invalid Filename UTF8 Conversion"),
                })?;

            let id = Uuid::parse_str(base).context(UuidError)?;

            let front: Front = serde_yaml::from_str(v[1]).context(YamlError)?;

            debug!("Creating Document {}", id);
            Ok(Some(Doc {
                front,
                id,
                updated_at: Utc::now(),
                content: String::from(v[2]),
            }))
        });

    future::ready(res)
}

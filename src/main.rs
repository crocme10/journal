use bb8_postgres::{bb8::Pool, PostgresConnectionManager};
use clap::{App, Arg};
use futures::future;
use futures::{stream, FutureExt, Stream, StreamExt, TryFutureExt, TryStreamExt};
use log::{debug, error, info, warn};
use snafu::{
    futures::try_future::TryFutureExt as SnafuTFE, futures::try_stream::TryStreamExt as SnafuTSE,
    NoneError, ResultExt,
};
use std::convert::Infallible;
use std::env;
use std::path::PathBuf;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use tokio::time::interval;
use tokio_postgres::tls::{NoTls, NoTlsStream};
use tokio_postgres::{
    config::Host, AsyncMessage, Client, Config, Connection, IsolationLevel, SimpleQueryMessage,
};
use uuid::Uuid;
use warp::{
    self,
    filters::sse::{self, ServerSentEvent},
    Filter, Reply,
};

mod error;
mod model;
mod watcher;

type Result<T, E = error::Error> = std::result::Result<T, E>;

enum Payload {
    Doc(model::Doc),
    Warning(String),
    Error(String),
}

#[tokio::main]
async fn main() -> Result<()> {
    let matches = App::new("journal")
        .version("0.1.0")
        .author("Matthieu Paindavoine <matt@area403.org>")
        .about("Webserver for markdown journal")
        .arg(
            Arg::with_name("assets")
                .multiple(true)
                .help("Directory to monitor for files to serve"),
        )
        .get_matches();

    let mut iter = matches
        .values_of("assets")
        .ok_or(snafu::NoneError)
        .context(error::UserError {
            details: String::from("Missing assets"),
        })?;

    pretty_env_logger::init();

    // Read the file ./postgres/database.env to extract user, password, and database name
    let dbenv = env::current_dir()
        .and_then(|d| Ok(d.join("sql").join("database.env")))
        .context(error::IOError)?;
    dotenv::from_path(dbenv.as_path())
        .or(Err(NoneError))
        .context(error::EnvError {
            details: String::from("database env"),
        })?;

    // Build the connection string
    let connstr = format!(
        "postgresql://{user}:{pwd}@localhost/{db}",
        user = dotenv::var("POSTGRES_USER")
            .or(Err(NoneError))
            .context(error::EnvError {
                details: String::from("POSTGRES_USER")
            })?,
        pwd = dotenv::var("POSTGRES_PASSWORD")
            .or(Err(NoneError))
            .context(error::EnvError {
                details: String::from("POSTGRES_PASSWORD")
            })?,
        db = dotenv::var("POSTGRES_DB")
            .or(Err(NoneError))
            .context(error::EnvError {
                details: String::from("POSTGRES_DB")
            })?,
    );

    debug!("DB Connection String: {}", connstr);

    // FIXME 1024 ??
    let (mut tx1, mut rx1) = mpsc::channel(1024);

    let pg_mgr = PostgresConnectionManager::new_from_stringlike(&connstr, tokio_postgres::NoTls)
        .context(error::DBConnError)?;

    let pool = Pool::builder()
        .build(pg_mgr)
        .await
        .context(error::DBConnError)?;

    // This thread receives documents, and inserts them in the database.
    tokio::spawn(async move {
        while let Some(payload) = rx1.recv().await {
            debug!("Received payload");
            match payload {
                Payload::Doc(doc) => {
                    doc2db(pool.clone(), doc)
                        .map_ok_or_else(
                            |err| error!("insert error: {}", err),
                            |id| info!("id: {}", id),
                        )
                        .await
                }
                Payload::Warning(warning) => {
                    future::ready({
                        warn!("Warning: {}", warning);
                    })
                    .await
                }
                Payload::Error(error) => {
                    future::ready({
                        error!("Error: {}", error);
                    })
                    .await
                }
            }
        }
    });

    let dir = PathBuf::from(iter.next().unwrap());

    // This thread monitors a directory, and sends documents that have changed through a channel.
    tokio::spawn(async move {
        let mut watcher = watcher::Watcher::new(dir.clone());

        if let Ok(mut stream) = watcher.doc_stream().context(error::IOError) {
            debug!("document stream available");
            loop {
                match stream.try_next().await {
                    Ok(opt_doc) => {
                        debug!("event: document");
                        if let Some(doc) = opt_doc {
                            tx1.send(Payload::Doc(doc)).await;
                        }
                    }
                    Err(err) => {
                        error!("Document Stream Error: {}", err);
                    }
                }
            }
        } else {
            error!("document stream error");
            tx1.send(Payload::Error(String::from("Could not get doc stream")))
                .await;
        }
        drop(watcher);
        info!("Terminating Watcher");
    });

    let feed = warp::path("feed").and(warp::get()).and_then(|| async move {
        let (client, mut connection) =
            connect_raw("postgresql://journaladmin:secret@postgres/journal")
                .await
                .unwrap();

        let (tx, rx) = futures::channel::mpsc::unbounded();
        let stream = stream::poll_fn(move |cx| connection.poll_message(cx)).map_err(|e| panic!(e));
        let connection = stream.forward(tx).map(|r| r.expect("stream forward"));
        tokio::spawn(connection);
        debug!("Spawned dedicated connection for postgres notifications");

        client
            .execute("LISTEN documents;", &[])
            .await
            .context(error::DBError)
            .unwrap();

        drop(client);

        let stream = make_stream(rx).unwrap();

        make_infallible(sse::reply(sse::keep_alive().stream(stream)))
    });

    let index = warp::fs::file("dist/index.html");
    let dir = warp::fs::dir("dist");
    let routes = feed.or(index).or(dir);
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
    Ok(())
}

fn make_stream(
    rx: futures::channel::mpsc::UnboundedReceiver<AsyncMessage>,
) -> Result<
    impl Stream<Item = Result<impl ServerSentEvent + Send + 'static, Infallible>> + Send + 'static,
    Infallible,
> {
    Ok(rx.filter_map(|m| match m {
        AsyncMessage::Notification(n) => {
            debug!("Received notification on channel: {}", n.channel());
            future::ready(Some(Ok((
                sse::event(String::from(n.channel())),
                sse::data(String::from(n.payload())),
            ))))
        }
        _ => {
            debug!("Received something on channel.");
            future::ready(None)
        }
    }))
}

fn make_infallible<T>(t: T) -> Result<T, Infallible> {
    Ok(t)
}

// async fn get_stream(
//     client: &Client,
//     mut connection: Connection<TcpStream, NoTlsStream>,
// ) -> Result<
//     impl Stream<Item = Result<impl ServerSentEvent + Send + 'static, Infallible>> + Send + 'static,
//     error::Error,
// > {
//     let (tx, rx) = futures::channel::mpsc::unbounded();
//     let stream = stream::poll_fn(move |cx| connection.poll_message(cx)).map_err(|e| panic!(e));
//     let connection = stream.forward(tx).map(|r| r.expect("stream forward"));
//     tokio::spawn(connection);
//     debug!("Spawned dedicated connection for postgres notifications");
//
//     client
//         .execute("LISTEN documents;", &[])
//         .await
//         .context(error::DBError)?;
//
//     debug!("Connection is closed: {:?}", client.is_closed());
//
//     //drop(client);
//
//     Ok(rx.filter_map(|m| match m {
//         AsyncMessage::Notification(n) => {
//             debug!("Received notification on channel: {}", n.channel());
//             future::ready(Some(Ok((
//                 sse::event(String::from(n.channel())),
//                 sse::data(String::from(n.payload())),
//             ))))
//         }
//         _ => {
//             debug!("Received something on channel.");
//             future::ready(None)
//         }
//     }))
// }

async fn doc2db(
    pool: Pool<PostgresConnectionManager<tokio_postgres::NoTls>>,
    doc: model::Doc,
) -> Result<String, error::Error> {
    let connection = pool.get().await.context(error::DBPoolError)?;

    let stmt = connection.prepare("SELECT * FROM create_document_with_id($1::UUID, $2::TEXT, $3::TEXT, $4::TEXT, $5::TEXT, $6::TEXT[], $7::TEXT, $8::DOC.DOC_KIND, $9::DOC.DOC_GENRE)").await.context(error::DBError)?;

    let row = connection
        .query_one(
            &stmt,
            &[
                &doc.id,
                &doc.front.title,
                &doc.front.outline,
                &doc.front.author,
                &doc.content,
                &doc.front.tags,
                &doc.front.image,
                &doc.front.kind,
                &doc.front.genre,
            ],
        )
        .await
        .context(error::DBError)?;

    Ok(format!("{}", row.get::<usize, Uuid>(0)))
}

async fn connect_raw(
    s: &str,
) -> Result<(Client, Connection<TcpStream, NoTlsStream>), error::Error> {
    let config = s.parse::<Config>().context(error::DBError)?;
    // Here we extract the host and port from the connection string.
    // Note that the port may not necessarily be explicitely specified,
    // the port 5432 is used by default.
    let host = config
        .get_hosts()
        .first()
        .ok_or(error::UserError {
            details: String::from("Missing host"),
        })
        .and_then(|h| match h {
            Host::Tcp(remote) => Ok(remote),
            Host::Unix(_) => Err(error::UserError {
                details: String::from("No local socket"),
            }),
        })
        .expect("host");
    let port = config
        .get_ports()
        .first()
        .ok_or(error::UserError {
            details: String::from("Missing port"),
        })
        .expect("port");

    let conn = format!("{}:{}", host, port);
    debug!("Connecting to {}", conn);
    let socket = TcpStream::connect(conn).await.context(error::IOError)?;
    config
        .connect_raw(socket, NoTls)
        .await
        .context(error::DBError)
}

fn sse_counter(counter: u64) -> Result<impl ServerSentEvent, Infallible> {
    Ok(warp::sse::data(counter))
}

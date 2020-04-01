use bb8_postgres::{bb8::Pool, PostgresConnectionManager};
use clap::{App, Arg};
use futures::executor::block_on;
use futures::future;
use futures::{stream, FutureExt, Stream, StreamExt, TryFutureExt, TryStreamExt};
use log::{debug, error, info, warn};
use snafu::{
    futures::try_future::TryFutureExt as SnafuTFE, futures::try_stream::TryStreamExt as SnafuTSE,
    ResultExt,
};
use std::path::PathBuf;
use tokio::sync::mpsc;
use uuid::Uuid;
use warp::{
    self,
    filters::sse::{self, ServerSentEvent},
    Filter,
};

use tokio::net::TcpStream;
use tokio_postgres::tls::{NoTls, NoTlsStream};
use tokio_postgres::{
    AsyncMessage, Client, Config, Connection, Error, IsolationLevel, SimpleQueryMessage,
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

    // FIXME 1024 ??
    let (mut tx1, mut rx1) = mpsc::channel(1024);

    // FIXME Hardcoded ??
    let pg_mgr = PostgresConnectionManager::new_from_stringlike(
        "postgresql://journaladmin:secret@postgres:5432/journal",
        tokio_postgres::NoTls,
    )
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
                    insert_doc(doc, pool.clone())
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

    let feed = warp::path("feed").and(warp::get()).map(|| {
        //let mut rt = tokio::runtime::Runtime::new().unwrap();
        let stream = block_on(async { get_stream().await.expect("Cannot get LISTEN stream") });
        sse::reply(sse::keep_alive().stream(stream))
    });

    let index = warp::fs::file("dist/index.html");
    let dir = warp::fs::dir("dist");
    let routes = feed.or(index).or(dir);
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
    Ok(())
}

async fn get_stream() -> Result<
    impl Stream<Item = Result<impl ServerSentEvent + Send + 'static, warp::Error>> + Send + 'static,
    error::Error,
> {
    let (client, mut connection) =
        connect_raw("postgresql://journaladmin:secret@postgres:5432/journal")
            .await
            .context(error::DBError)?;

    let (tx, rx) = futures::channel::mpsc::unbounded();
    let stream = stream::poll_fn(move |cx| connection.poll_message(cx)).map_err(|e| panic!(e));
    let connection = stream.forward(tx).map(|r| r.expect("stream forward"));
    tokio::spawn(connection);
    debug!("Spawned dedicated connection for postgres notifications");

    client
        //.execute("LISTEN documents", &[])
        .query_one("LISTEN documents;", &[])
        .await
        .context(error::DBError)?;

    debug!("Connection is closed: {:?}", client.is_closed());

    drop(client);

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

fn insert_doc(
    doc: model::Doc,
    pool: Pool<PostgresConnectionManager<tokio_postgres::NoTls>>,
) -> impl future::TryFuture<Ok = String, Error = error::Error> + 'static {
    select(pool, doc)
}

async fn select(
    pool: Pool<PostgresConnectionManager<tokio_postgres::NoTls>>,
    doc: model::Doc,
) -> Result<String, error::Error> {
    let connection = pool.get().await.context(error::DBPoolError)?;

    let stmt = connection.prepare("SELECT * FROM doc.create_document_with_id($1::UUID, $2::TEXT, $3::TEXT, $4::TEXT, $5::TEXT, $6::TEXT[], $7::TEXT, $8::DOC.DOC_KIND, $9::DOC.DOC_GENRE)").await.context(error::DBError)?;

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

async fn connect_raw(s: &str) -> Result<(Client, Connection<TcpStream, NoTlsStream>), Error> {
    let socket = TcpStream::connect("10.0.3.14:5432")
        .await
        .expect("socket connection");
    let config = s.parse::<Config>().expect("config");
    config.connect_raw(socket, NoTls).await
}

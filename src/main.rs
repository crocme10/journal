use bb8_postgres::{bb8::Pool, PostgresConnectionManager};
use clap::{App, Arg};
use futures::{future, stream, FutureExt, Stream, StreamExt, TryFutureExt, TryStreamExt};
use juniper;
use log::{debug, error, info, warn};
use snafu::{NoneError, ResultExt};
use std::convert::Infallible;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_postgres::tls::{NoTls, NoTlsStream};
use tokio_postgres::{config::Host, AsyncMessage, Client, Config, Connection};
use uuid::Uuid;
use warp::{
    self,
    filters::sse::{self, ServerSentEvent},
    filters::BoxedFilter,
    path, Filter,
};

mod error;
mod gql;
mod model;
mod watcher;

type Schema = juniper::RootNode<
    'static,
    gql::Query,
    juniper::EmptyMutation<gql::Context>,
    juniper::EmptySubscription<gql::Context>,
>;

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
            Arg::with_name("dist")
                .index(1)
                .help("Directory to serve static file from."),
        )
        .arg(
            Arg::with_name("assets")
                .index(2)
                .help("Directory to monitor for files to serve"),
        )
        .get_matches();

    let mut assets = matches
        .values_of("assets")
        .ok_or(snafu::NoneError)
        .context(error::UserError {
            details: String::from("Missing assets"),
        })?;

    let mut dist = matches
        .values_of("dist")
        .ok_or(snafu::NoneError)
        .context(error::UserError {
            details: String::from("Missing dist"),
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
        "postgresql://{user}:{pwd}@{host}/{db}",
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
        host = dotenv::var("POSTGRES_HOST")
            .or(Err(NoneError))
            .context(error::EnvError {
                details: String::from("POSTGRES_HOST")
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

    let assets_dir = PathBuf::from(assets.next().unwrap());

    debug!("Monitoring {}", assets_dir.display());

    // This thread monitors a directory, and sends documents that have changed through a channel.
    tokio::spawn(async move {
        let mut watcher = watcher::Watcher::new(assets_dir.clone());

        if let Ok(mut stream) = watcher.doc_stream().context(error::IOError) {
            debug!("Document Stream available");
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

    // TODO Move feed function to separate function to keep main small
    let feed = warp::path("feed").and(warp::get()).and_then(|| async move {
        debug!("Entering feed");

        let (tx, rx) = futures::channel::mpsc::unbounded();

        let (client, mut connection) =
            connect_raw("postgresql://journaladmin:secret@localhost/journal")
                .await
                .unwrap();

        let stream = stream::poll_fn(move |cx| connection.poll_message(cx)).map_err(|e| panic!(e));

        let connection = stream.forward(tx).map(|r| r.unwrap());

        tokio::spawn(connection);

        debug!("execute LISTEN");

        client
            .execute("LISTEN documents;", &[])
            .await
            .context(error::DBError)
            .unwrap();

        debug!("LISTEN");

        tokio::spawn(async move {
            loop {}
            drop(client);
        });

        debug!("After spawn");

        let stream = make_stream(rx).unwrap();

        make_infallible(sse::reply(sse::keep_alive().stream(stream)))
    });

    let state = warp::any().map(|| {
        let client = futures::executor::block_on(connect(
            "postgresql://journaladmin:secret@localhost/journal",
        ))
        .expect("db connection");
        gql::Context { client }
    });

    let graphql_filter = juniper_warp::make_graphql_filter(schema(), state.boxed());

    let gql_index = warp::path("graphiql")
        .and(warp::path::end())
        .and(warp::get())
        .and(juniper_warp::graphiql_filter("/graphql"));

    let gql_query = warp::path("graphql").and(graphql_filter);

    let dist_path = PathBuf::from(dist.next().unwrap());
    let index_path = dist_path.join("index.html");
    let index = warp::fs::file(index_path);
    let dir = warp::fs::dir(dist_path);
    let routes = gql_index.or(gql_query).or(feed).or(dir).or(index);

    // Read the file ./server.env to extract TLS information and port
    let serverenv = PathBuf::from("server.env");
    dotenv::from_path(serverenv.as_path())
        .or(Err(NoneError))
        .context(error::EnvError {
            details: String::from("server env"),
        })?;

    let cert_path = dotenv::var("CERT_PATH")
        .or(Err(NoneError))
        .context(error::EnvError {
            details: String::from("CERT_PATH")
        })?;
    let key_path = dotenv::var("KEY_PATH")
        .or(Err(NoneError))
        .context(error::EnvError {
            details: String::from("KEY_PATH")
        })?;
    let port = dotenv::var("SERVER_PORT")
        .or(Err(NoneError))
        .context(error::EnvError {
            details: String::from("SERVER_PORT")
        })?
        .parse::<u16>()
        .context(error::ParseError)?;

    warp::serve(routes)
        // .tls()
        // .cert_path(cert_path)
        // .key_path(key_path)
        .run(([127, 0, 0, 1], port))
        .await;
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

async fn doc2db(
    pool: Pool<PostgresConnectionManager<tokio_postgres::NoTls>>,
    doc: model::Doc,
) -> Result<String, error::Error> {
    let connection = pool.get().await.context(error::DBPoolError)?;

    let stmt = connection.prepare("SELECT * FROM create_document_with_id($1::UUID, $2::TEXT, $3::TEXT, $4::TEXT, $5::TEXT, $6::TEXT[], $7::TEXT, $8::KIND, $9::GENRE)").await.context(error::DBError)?;

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

async fn connect(s: &str) -> Result<Client, error::Error> {
    let (client, conn) = connect_raw(s).await?;
    let conn = conn.map(|r| r.unwrap());
    tokio::spawn(conn);
    Ok(client)
}

fn schema() -> Schema {
    Schema::new(
        gql::Query,
        juniper::EmptyMutation::<gql::Context>::new(),
        juniper::EmptySubscription::<gql::Context>::new(),
    )
}

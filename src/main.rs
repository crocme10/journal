use chrono::Utc;
use futures::{future, TryFutureExt, TryStreamExt};
use juniper;
use log::{debug, error, info, warn};
use snafu::{NoneError, ResultExt};
use sqlx::postgres::{PgPool, PgQueryAs};
use tokio::sync::mpsc;
use uuid::Uuid;
use warp::{
    self,
    filters::sse::{self, ServerSentEvent},
    Filter,
};

mod config;
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
    let _dotenv = dotenv::dotenv()
        .or(Err(NoneError))
        .context(error::EnvError {
            details: String::from("dot env"),
        })?;

    let config = config::Config::new()?;

    pretty_env_logger::init();

    let connstr = dotenv::var("DATABASE_URL")
        .or(Err(NoneError))
        .context(error::EnvError {
            details: String::from("DATABASE_URL"),
        })?
        .to_string();

    debug!("DB Connection String: {}", connstr);

    // FIXME 1024 ??
    let (mut tx1, mut rx1) = mpsc::channel(1024);

    let pool = PgPool::builder()
        .max_size(5) // maximum number of connections in the pool
        .build(&connstr)
        .await
        .context(error::DBConnError)?;

    let pool1 = pool.clone();
    // This thread receives documents, and inserts them in the database.
    tokio::spawn(async move {
        while let Some(payload) = rx1.recv().await {
            debug!("Received payload");
            match payload {
                Payload::Doc(doc) => {
                    doc2db(pool1.clone(), doc)
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

    // This thread monitors a directory, and sends documents that have changed through a channel.
    debug!("Monitoring {}", config.assets_path.display());
    let assets_path = config.assets_path.clone();
    tokio::spawn(async move {
        let mut watcher = watcher::Watcher::new(assets_path);

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

    // let connstr = Arc::new(connstr);
    // let connstr1 = Arc::clone(&connstr);

    // TODO Move feed function to separate function to keep main small
    // let feed = warp::path("feed").and(warp::get()).and_then(move || {
    //     let connstr = Arc::clone(&connstr1);
    //     async move {
    //         let stream = feed_stream(&connstr).await.unwrap();
    //         make_infallible(sse::reply(sse::keep_alive().stream(stream)))
    //     }
    // });

    // let connstr2 = Arc::clone(&connstr);

    let state = warp::any().map(move || gql::Context { pool: pool.clone() });

    let graphql_filter = juniper_warp::make_graphql_filter(schema(), state.boxed());

    let gql_index = warp::path("graphiql")
        .and(warp::path::end())
        .and(warp::get())
        .and(juniper_warp::graphiql_filter("/graphql"));

    let gql_query = warp::path("graphql").and(graphql_filter);

    let routes = gql_index.or(gql_query);

    info!("Serving journal on 0.0.0.0:{}", config.port);

    warp::serve(routes)
        // .tls()
        // .cert_path(cert_path)
        // .key_path(key_path)
        .run(([0, 0, 0, 0], config.port))
        .await;
    Ok(())
}

// async fn feed_stream(
//     connstr: &str,
// ) -> Result<
//     impl Stream<Item = Result<impl ServerSentEvent + Send + 'static, Infallible>> + Send + 'static,
//     Infallible,
// > {
//     debug!("Entering feed");
//
//     let (tx, rx) = futures::channel::mpsc::unbounded();
//
//     let (client, mut connection) = connect_raw(connstr).await.unwrap();
//
//     let stream = stream::poll_fn(move |cx| connection.poll_message(cx)).map_err(|e| panic!(e));
//
//     let connection = stream.forward(tx).map(|r| r.unwrap());
//
//     tokio::spawn(connection);
//
//     debug!("execute LISTEN");
//
//     client
//         .execute("LISTEN documents;", &[])
//         .await
//         .context(error::DBError)
//         .unwrap();
//
//     debug!("LISTEN");
//
//     tokio::spawn(async move {
//         loop {}
//         drop(client);
//     });
//
//     debug!("After spawn");
//
//     make_stream(rx)
// }

// fn make_stream(
//     rx: futures::channel::mpsc::UnboundedReceiver<PgNotification>,
// ) -> Result<
//     impl Stream<Item = Result<impl ServerSentEvent + Send + 'static, Infallible>> + Send + 'static,
//     Infallible,
// > {
//     Ok(rx.filter_map(|m| match m {
//         PgNotification::Notification(n) => {
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
//
// fn make_infallible<T>(t: T) -> Result<T, Infallible> {
//     Ok(t)
// }

async fn doc2db(pool: PgPool, doc: model::Doc) -> Result<String, error::Error> {
    //let conn = pool.acquire().await.context(error::DBConnError)?;

    let (id, created_at): (Uuid, chrono::DateTime<Utc>) = sqlx::query_as(
        "SELECT _id::UUID, _created_at::TIMESTAMPTZ FROM create_document_with_id(
            $1::UUID, $2::TEXT, $3::TEXT, $4::TEXT, $5::TEXT,
            $6::TEXT[], $7::TEXT, $8::KIND, $9::GENRE)",
    )
    .bind(&doc.id)
    .bind(&doc.front.title)
    .bind(&doc.front.outline)
    .bind(&doc.front.author)
    .bind(&doc.content)
    .bind(&doc.front.tags)
    .bind(&doc.front.image)
    .bind(&doc.front.kind)
    .bind(&doc.front.genre)
    .fetch_one(&pool)
    .await
    .context(error::DBConnError)?;

    Ok(format!("{}: {}", id, created_at))
}

// async fn connect_raw(
//     s: &str,
// ) -> Result<(Client, Connection<TcpStream, NoTlsStream>), error::Error> {
//     let config = s.parse::<Config>().context(error::DBError)?;
//     // Here we extract the host and port from the connection string.
//     // Note that the port may not necessarily be explicitely specified,
//     // the port 5432 is used by default.
//     let host = config
//         .get_hosts()
//         .first()
//         .ok_or(error::UserError {
//             details: String::from("Missing host"),
//         })
//         .and_then(|h| match h {
//             Host::Tcp(remote) => Ok(remote),
//             Host::Unix(_) => Err(error::UserError {
//                 details: String::from("No local socket"),
//             }),
//         })
//         .expect("host");
//     let port = config
//         .get_ports()
//         .first()
//         .ok_or(error::UserError {
//             details: String::from("Missing port"),
//         })
//         .expect("port");
//
//     let conn = format!("{}:{}", host, port);
//     debug!("Connecting to {}", conn);
//     let socket = TcpStream::connect(conn).await.context(error::IOError)?;
//     config
//         .connect_raw(socket, NoTls)
//         .await
//         .context(error::DBError)
// }
//
// async fn connect(s: &str) -> Result<Client, error::Error> {
//     let (client, conn) = connect_raw(s).await?;
//     let conn = conn.map(|r| r.unwrap());
//     tokio::spawn(conn);
//     Ok(client)
// }

fn schema() -> Schema {
    Schema::new(
        gql::Query,
        juniper::EmptyMutation::<gql::Context>::new(),
        juniper::EmptySubscription::<gql::Context>::new(),
    )
}

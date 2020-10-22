use async_trait::async_trait;
use chrono::{DateTime, Utc};
use slog::{debug, info, o, Logger};
use snafu::ResultExt;
use sqlx::error::DatabaseError;
use sqlx::pool::PoolConnection;
use sqlx::postgres::{PgError, PgQueryAs, PgRow};
use sqlx::row::{FromRow, Row};
use sqlx::{PgConnection, PgPool};
use std::convert::TryFrom;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use super::model;
use super::Db;
use crate::error;

#[derive(sqlx::Type)]
#[sqlx(rename = "kind", rename_all = "lowercase")]
pub enum DocKind {
    Doc,
    Post,
}

impl From<DocKind> for model::DocKind {
    fn from(kind: DocKind) -> Self {
        match kind {
            DocKind::Doc => model::DocKind::Doc,
            DocKind::Post => model::DocKind::Post,
        }
    }
}

impl From<&model::DocKind> for DocKind {
    fn from(kind: &model::DocKind) -> Self {
        match kind {
            model::DocKind::Doc => DocKind::Doc,
            model::DocKind::Post => DocKind::Post,
        }
    }
}

#[derive(sqlx::Type)]
#[sqlx(rename = "genre", rename_all = "lowercase")]
pub enum DocGenre {
    Tutorial,
    Howto,
    Background,
    Reference,
}

impl From<DocGenre> for model::DocGenre {
    fn from(genre: DocGenre) -> Self {
        match genre {
            DocGenre::Tutorial => model::DocGenre::Tutorial,
            DocGenre::Howto => model::DocGenre::Howto,
            DocGenre::Background => model::DocGenre::Background,
            DocGenre::Reference => model::DocGenre::Reference,
        }
    }
}

impl From<&model::DocGenre> for DocGenre {
    fn from(genre: &model::DocGenre) -> Self {
        match genre {
            model::DocGenre::Tutorial => DocGenre::Tutorial,
            model::DocGenre::Howto => DocGenre::Howto,
            model::DocGenre::Background => DocGenre::Background,
            model::DocGenre::Reference => DocGenre::Reference,
        }
    }
}

/// A document registered with the application (Postgres version)
pub struct DocEntity {
    pub id: model::EntityId,
    pub title: String,
    pub outline: String,
    pub author: String,
    pub tags: Vec<String>,
    pub image: String,
    pub kind: DocKind,
    pub genre: DocGenre,
    pub content: String,
    pub updated_at: DateTime<Utc>,
}

impl<'c> FromRow<'c, PgRow<'c>> for DocEntity {
    fn from_row(row: &PgRow<'c>) -> Result<Self, sqlx::Error> {
        Ok(DocEntity {
            id: row.get(0),
            title: row.get(1),
            outline: row.get(2),
            author: row.get(3),
            tags: row.get(4),
            image: row.get(5),
            kind: row.get(6),
            genre: row.get(7),
            content: row.get(8),
            updated_at: row.get(10),
        })
    }
}

impl From<DocEntity> for model::DocEntity {
    fn from(pg: DocEntity) -> Self {
        let DocEntity {
            id,
            title,
            outline,
            author,
            tags,
            image,
            kind,
            genre,
            content,
            updated_at,
        } = pg;

        model::DocEntity {
            id,
            title,
            outline,
            author,
            tags,
            image,
            kind: model::DocKind::from(kind),
            genre: model::DocGenre::from(genre),
            updated_at,
            content,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DocCreationAck {
    pub id: model::EntityId,
    pub created_at: DateTime<Utc>,
}

impl<'c> FromRow<'c, PgRow<'c>> for DocCreationAck {
    fn from_row(row: &PgRow<'c>) -> Result<Self, sqlx::Error> {
        Ok(DocCreationAck {
            id: row.get(0),
            created_at: row.get(1),
        })
    }
}

impl From<DocCreationAck> for model::DocCreationAck {
    fn from(pg: DocCreationAck) -> Self {
        let DocCreationAck { id, created_at } = pg;

        model::DocCreationAck { id, created_at }
    }
}

/// Open a connection to a database
pub async fn connect(db_url: &str) -> sqlx::Result<PgPool> {
    let pool = PgPool::new(db_url).await?;
    Ok(pool)
}

impl TryFrom<&PgError> for model::ProvideError {
    type Error = ();

    /// Attempt to convert a Postgres error into a generic ProvideError
    ///
    /// Unexpected cases will be bounced back to the caller for handling
    ///
    /// * [Postgres Error Codes](https://www.postgresql.org/docs/current/errcodes-appendix.html)
    fn try_from(pg_err: &PgError) -> Result<Self, Self::Error> {
        let provider_err = match pg_err.code().unwrap() {
            "23505" => model::ProvideError::UniqueViolation {
                details: pg_err.details().unwrap().to_owned(),
            },
            code if code.starts_with("23") => model::ProvideError::ModelViolation {
                details: pg_err.message().to_owned(),
            },
            _ => return Err(()),
        };

        Ok(provider_err)
    }
}

#[async_trait]
impl Db for PgPool {
    type Conn = PoolConnection<PgConnection>;

    async fn conn(&self) -> Result<Self::Conn, sqlx::Error> {
        self.acquire().await
    }
}

#[async_trait]
impl model::ProvideData for PgConnection {
    async fn get_all_documents(&mut self) -> model::ProvideResult<Vec<model::DocEntity>> {
        let docs: Vec<DocEntity> = sqlx::query_as(
            r#"
SELECT *
FROM documents
ORDER BY updated_at
            "#,
        )
        .fetch_all(self)
        .await?;

        let docs = docs
            .into_iter()
            .map(model::DocEntity::from)
            .collect::<Vec<_>>();

        Ok(docs)
    }

    async fn get_document_by_id(
        &mut self,
        id: model::EntityId,
    ) -> model::ProvideResult<Option<model::DocEntity>> {
        let doc: Option<DocEntity> = sqlx::query_as(
            r#"
SELECT *
FROM documents
WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(self)
        .await?;

        match doc {
            None => Ok(None),
            Some(doc) => {
                let doc = model::DocEntity::from(doc);
                Ok(Some(doc))
            }
        }
    }

    async fn create_or_update_document(
        &mut self,
        doc: &model::DocEntity,
    ) -> model::ProvideResult<model::DocCreationAck> {
        let kind = DocKind::from(&doc.kind);
        let genre = DocGenre::from(&doc.genre);
        let resp: DocCreationAck = sqlx::query_as(
            "SELECT _id::UUID, _created_at::TIMESTAMPTZ FROM create_document_with_id(
            $1::UUID, $2::TEXT, $3::TEXT, $4::TEXT, $5::TEXT,
            $6::TEXT[], $7::TEXT, $8::KIND, $9::GENRE)",
        )
        .bind(&doc.id)
        .bind(&doc.title)
        .bind(&doc.outline)
        .bind(&doc.author)
        .bind(&doc.content)
        .bind(&doc.tags)
        .bind(&doc.image)
        .bind(&kind)
        .bind(&genre)
        .fetch_one(self)
        .await?;

        Ok(model::DocCreationAck::from(resp))
    }
}

pub async fn init_db(conn_str: &str, logger: Logger) -> Result<(), error::Error> {
    info!(logger, "Initializing  DB @ {}", conn_str);
    migration_down(conn_str, &logger).await?;
    migration_up(conn_str, &logger).await?;
    Ok(())
}

pub async fn migration_up(conn_str: &str, logger: &Logger) -> Result<(), error::Error> {
    let clogger = logger.new(o!("database" => String::from(conn_str)));
    debug!(clogger, "Movine Up");
    // This is essentially running 'psql $DATABASE_URL < db/init.sql', and logging the
    // psql output.
    // FIXME This relies on a command psql, which is not desibable.
    // We could alternatively try to use sqlx...
    // There may be a tool for doing migrations.
    let mut cmd = Command::new("movine");
    cmd.env("DATABASE_URL", conn_str);
    cmd.arg("up");
    cmd.stdout(Stdio::piped());

    let mut child = cmd.spawn().context(error::TokioIOError {
        msg: String::from("Failed to execute movine"),
    })?;

    let stdout = child.stdout.take().ok_or(error::Error::MiscError {
        msg: String::from("child did not have a handle to stdout"),
    })?;

    let mut reader = BufReader::new(stdout).lines();

    // Ensure the child process is spawned in the runtime so it can
    // make progress on its own while we await for any output.
    tokio::spawn(async {
        // FIXME Need to do something about logging this and returning an error.
        let _status = child.await.expect("child process encountered an error");
        // println!("child status was: {}", status);
    });
    debug!(clogger, "Spawned migration up");

    while let Some(line) = reader.next_line().await.context(error::TokioIOError {
        msg: String::from("Could not read from piped output"),
    })? {
        debug!(clogger, "movine: {}", line);
    }

    Ok(())
}

pub async fn migration_down(conn_str: &str, logger: &Logger) -> Result<(), error::Error> {
    let clogger = logger.new(o!("database" => String::from(conn_str)));
    debug!(clogger, "Movine Down");
    // This is essentially running 'psql $DATABASE_URL < db/init.sql', and logging the
    // psql output.
    // FIXME This relies on a command psql, which is not desibable.
    // We could alternatively try to use sqlx...
    // There may be a tool for doing migrations.
    let mut cmd = Command::new("movine");
    cmd.env("DATABASE_URL", conn_str);
    cmd.arg("down");
    cmd.stdout(Stdio::piped());

    let mut child = cmd.spawn().context(error::TokioIOError {
        msg: String::from("Failed to execute movine"),
    })?;

    let stdout = child.stdout.take().ok_or(error::Error::MiscError {
        msg: String::from("child did not have a handle to stdout"),
    })?;

    let mut reader = BufReader::new(stdout).lines();

    // Ensure the child process is spawned in the runtime so it can
    // make progress on its own while we await for any output.
    tokio::spawn(async {
        // FIXME Need to do something about logging this and returning an error.
        let _status = child.await.expect("child process encountered an error");
        // println!("child status was: {}", status);
    });
    debug!(clogger, "Spawned migration down");

    while let Some(line) = reader.next_line().await.context(error::TokioIOError {
        msg: String::from("Could not read from piped output"),
    })? {
        debug!(clogger, "movine: {}", line);
    }

    Ok(())
}

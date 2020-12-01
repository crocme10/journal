use async_trait::async_trait;
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

// This should match the information in return_document_type
impl<'c> FromRow<'c, PgRow<'c>> for model::DocEntity {
    fn from_row(row: &PgRow<'c>) -> Result<Self, sqlx::Error> {
        let author = model::AuthorEntity {
            id: Some(row.try_get(3)?),
            fullname: row.try_get(4)?,
            resource: row.try_get(5)?,
        };

        let image_author = model::AuthorEntity {
            id: Some(row.try_get(10)?),
            fullname: row.try_get(11)?,
            resource: row.try_get(12)?,
        };

        let image = model::ImageEntity {
            id: Some(row.try_get(8)?),
            title: row.try_get(9)?,
            author: image_author,
            resource: row.try_get(13)?,
        };

        Ok(model::DocEntity {
            id: row.try_get(0)?,
            title: row.try_get(1)?,
            outline: row.try_get(2)?,
            author,
            tags: row.try_get(7)?,
            image,
            kind: row.try_get(14)?,
            genre: row.try_get(15)?,
            content: row.try_get(6)?,
            created_at: row.try_get(16)?,
            updated_at: row.try_get(17)?,
        })
    }
}

// This should match the information in return_document_type
impl<'c> FromRow<'c, PgRow<'c>> for model::ShortDocEntity {
    fn from_row(row: &PgRow<'c>) -> Result<Self, sqlx::Error> {
        let author = model::AuthorEntity {
            id: Some(row.try_get(3)?),
            fullname: row.try_get(4)?,
            resource: row.try_get(5)?,
        };

        let image_author = model::AuthorEntity {
            id: Some(row.try_get(9)?),
            fullname: row.try_get(10)?,
            resource: row.try_get(11)?,
        };

        let image = model::ImageEntity {
            id: Some(row.try_get(7)?),
            title: row.try_get(8)?,
            author: image_author,
            resource: row.try_get(12)?,
        };

        Ok(model::ShortDocEntity {
            id: row.try_get(0)?,
            title: row.try_get(1)?,
            outline: row.try_get(2)?,
            author,
            tags: row.try_get(6)?,
            image,
            kind: row.try_get(13)?,
            genre: row.try_get(14)?,
            created_at: row.try_get(15)?,
            updated_at: row.try_get(16)?,
        })
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
impl model::ProvideJournal for PgConnection {
    async fn get_all_documents(
        &mut self,
        kind: model::DocKind,
    ) -> model::ProvideResult<Vec<model::ShortDocEntity>> {
        let docs: Vec<model::ShortDocEntity> =
            sqlx::query_as(r#"SELECT * FROM main.list_documents($1)"#)
                .bind(kind)
                .fetch_all(self)
                .await?;

        Ok(docs)
    }

    async fn get_document_by_id(
        &mut self,
        id: model::EntityId,
    ) -> model::ProvideResult<Option<model::DocEntity>> {
        let doc: Option<model::DocEntity> =
            sqlx::query_as(r#"SELECT * FROM main.get_document_by_id($1)"#)
                .bind(id)
                .fetch_optional(self)
                .await?;

        Ok(doc)
    }

    async fn create_or_update_document(
        &mut self,
        doc: &model::DocEntity,
    ) -> model::ProvideResult<model::DocEntity> {
        let resp: model::DocEntity = sqlx::query_as(
            "SELECT * FROM main.create_document_with_id(
            $1::UUID, $2::TEXT, $3::TEXT, $4::TEXT, $5::TEXT,
            $6::TEXT, $7::TEXT[], $8::TEXT, $9::TEXT,
            $10::TEXT, $11::TEXT, $12::MAIN.KIND, $13::MAIN.GENRE)",
        )
        .bind(&doc.id)
        .bind(&doc.title)
        .bind(&doc.outline)
        .bind(&doc.author.fullname)
        .bind(&doc.author.resource)
        .bind(&doc.content)
        .bind(&doc.tags)
        .bind(&doc.image.title)
        .bind(&doc.image.author.fullname)
        .bind(&doc.image.author.resource)
        .bind(&doc.image.resource)
        .bind(&doc.kind)
        .bind(&doc.genre)
        .fetch_one(self)
        .await?;

        Ok(resp)
    }

    async fn get_all_documents_by_query(
        &mut self,
        query: &str,
    ) -> model::ProvideResult<Vec<model::ShortDocEntity>> {
        let docs: Vec<model::ShortDocEntity> =
            sqlx::query_as(r#"SELECT * FROM main.search_documents_by_query($1)"#)
                .bind(query)
                .fetch_all(self)
                .await?;

        Ok(docs)
    }

    async fn get_all_documents_by_tag(
        &mut self,
        tag: &str,
    ) -> model::ProvideResult<Vec<model::ShortDocEntity>> {
        let docs: Vec<model::ShortDocEntity> =
            sqlx::query_as(r#"SELECT * FROM main.search_documents_by_tag($1)"#)
                .bind(tag)
                .fetch_all(self)
                .await?;

        Ok(docs)
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

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use snafu::Snafu;
use std::convert::TryFrom;
use uuid::Uuid;

pub type EntityId = Uuid;

#[derive(Debug, sqlx::Type)]
#[sqlx(rename = "kind", rename_all = "lowercase")]
pub enum DocKind {
    Doc,
    Post,
}

#[derive(Debug, sqlx::Type)]
#[sqlx(rename = "genre", rename_all = "lowercase")]
pub enum DocGenre {
    Tutorial,
    Howto,
    Background,
    Reference,
}

#[derive(Debug)]
pub struct AuthorEntity {
    pub id: Option<EntityId>,
    pub fullname: String,
    pub resource: String,
}

#[derive(Debug)]
pub struct ImageEntity {
    pub id: Option<EntityId>,
    pub title: String,
    pub author: AuthorEntity,
    pub resource: String,
}

#[derive(Debug)]
pub struct ShortDocEntity {
    pub id: EntityId,
    pub title: String,
    pub outline: String,
    pub author: AuthorEntity,
    pub tags: Vec<String>,
    pub image: ImageEntity,
    pub kind: DocKind,
    pub genre: DocGenre,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct DocEntity {
    pub id: EntityId,
    pub title: String,
    pub outline: String,
    pub author: AuthorEntity,
    pub tags: Vec<String>,
    pub image: ImageEntity,
    pub kind: DocKind,
    pub genre: DocGenre,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[async_trait]
pub trait ProvideJournal {
    async fn get_all_documents(&mut self) -> ProvideResult<Vec<ShortDocEntity>>;

    async fn get_document_by_id(&mut self, id: EntityId) -> ProvideResult<Option<DocEntity>>;

    async fn create_or_update_document(&mut self, doc: &DocEntity) -> ProvideResult<DocEntity>;

    async fn get_all_documents_by_query(
        &mut self,
        query: &str,
    ) -> ProvideResult<Vec<ShortDocEntity>>;

    async fn get_all_documents_by_tag(&mut self, tag: &str) -> ProvideResult<Vec<ShortDocEntity>>;
}

pub type ProvideResult<T> = Result<T, ProvideError>;

/// An error returned by a provider
#[derive(Debug, Snafu)]
pub enum ProvideError {
    /// The requested entity does not exist
    #[snafu(display("Entity does not exist"))]
    #[snafu(visibility(pub))]
    NotFound,

    /// The operation violates a uniqueness constraint
    #[snafu(display("Operation violates uniqueness constraint: {}", details))]
    #[snafu(visibility(pub))]
    UniqueViolation { details: String },

    /// The requested operation violates the data model
    #[snafu(display("Operation violates model: {}", details))]
    #[snafu(visibility(pub))]
    ModelViolation { details: String },

    /// The requested operation violates the data model
    #[snafu(display("UnHandled Error: {}", source))]
    #[snafu(visibility(pub))]
    UnHandledError { source: sqlx::Error },
}

impl From<sqlx::Error> for ProvideError {
    /// Convert a SQLx error into a provider error
    ///
    /// For Database errors we attempt to downcast
    ///
    /// FIXME(RFC): I have no idea if this is sane
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::RowNotFound => ProvideError::NotFound,
            sqlx::Error::Database(db_err) => {
                if let Some(pg_err) = db_err.try_downcast_ref::<sqlx::postgres::PgError>() {
                    if let Ok(provide_err) = ProvideError::try_from(pg_err) {
                        provide_err
                    } else {
                        ProvideError::UnHandledError {
                            source: sqlx::Error::Database(db_err),
                        }
                    }
                } else {
                    ProvideError::UnHandledError {
                        source: sqlx::Error::Database(db_err),
                    }
                }
            }
            _ => ProvideError::UnHandledError { source: e },
        }
    }
}

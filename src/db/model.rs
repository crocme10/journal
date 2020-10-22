use async_trait::async_trait;
use chrono::{DateTime, Utc};
use snafu::Snafu;
use std::convert::TryFrom;
use uuid::Uuid;

pub type EntityId = Uuid;

#[derive(Debug, Clone)]
pub enum DocKind {
    Doc,
    Post,
}

#[derive(Debug, Clone)]
pub enum DocGenre {
    Tutorial,
    Howto,
    Background,
    Reference,
}

/// A document stored in the database
#[derive(Debug, Clone)]
pub struct DocEntity {
    pub id: EntityId,
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

#[derive(Debug, Clone)]
pub struct DocCreationAck {
    pub id: EntityId,
    pub created_at: DateTime<Utc>,
}

// From sqlx realworld example
#[async_trait]
pub trait ProvideData {
    async fn get_all_documents(&mut self) -> ProvideResult<Vec<DocEntity>>;

    async fn get_document_by_id(&mut self, id: EntityId) -> ProvideResult<Option<DocEntity>>;

    async fn create_or_update_document(&mut self, doc: &DocEntity)
        -> ProvideResult<DocCreationAck>;
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

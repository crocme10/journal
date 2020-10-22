use chrono::{DateTime, Utc};
use juniper::futures::TryFutureExt;
use juniper::{GraphQLEnum, GraphQLInputObject, GraphQLObject};
use serde::{Deserialize, Serialize};
use slog::info;
use snafu::ResultExt;
use sqlx::Connection;
use std::convert::TryFrom;
use uuid::Uuid;

use crate::api::gql::Context;
use crate::db::model as db;
use crate::db::model::ProvideData;
use crate::db::Db;
use crate::error;

#[derive(Debug, PartialEq, Serialize, Deserialize, GraphQLEnum)]
#[serde(rename_all = "camelCase")]
pub enum DocKind {
    Doc,
    Post,
}

impl From<db::DocKind> for DocKind {
    fn from(kind: db::DocKind) -> Self {
        match kind {
            db::DocKind::Doc => DocKind::Doc,
            db::DocKind::Post => DocKind::Post,
        }
    }
}

impl From<DocKind> for db::DocKind {
    fn from(kind: DocKind) -> Self {
        match kind {
            DocKind::Doc => db::DocKind::Doc,
            DocKind::Post => db::DocKind::Post,
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, GraphQLEnum)]
#[serde(rename_all = "camelCase")]
pub enum DocGenre {
    Tutorial,
    Howto,
    Background,
    Reference,
}

impl From<db::DocGenre> for DocGenre {
    fn from(genre: db::DocGenre) -> Self {
        match genre {
            db::DocGenre::Tutorial => DocGenre::Tutorial,
            db::DocGenre::Howto => DocGenre::Howto,
            db::DocGenre::Background => DocGenre::Background,
            db::DocGenre::Reference => DocGenre::Reference,
        }
    }
}

impl From<DocGenre> for db::DocGenre {
    fn from(genre: DocGenre) -> Self {
        match genre {
            DocGenre::Tutorial => db::DocGenre::Tutorial,
            DocGenre::Howto => db::DocGenre::Howto,
            DocGenre::Background => db::DocGenre::Background,
            DocGenre::Reference => db::DocGenre::Reference,
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, GraphQLObject)]
#[serde(rename_all = "camelCase")]
pub struct Front {
    pub title: String,
    #[serde(rename = "abstract")]
    pub outline: String,
    pub author: String,
    pub tags: Vec<String>,
    pub image: String,
    #[serde(default = "default_kind")]
    pub kind: DocKind,
    #[serde(default = "default_genre")]
    pub genre: DocGenre,
    pub updated_at: DateTime<Utc>,
}

pub fn default_kind() -> DocKind {
    DocKind::Doc
}

pub fn default_genre() -> DocGenre {
    DocGenre::Tutorial
}

#[derive(Debug, PartialEq, Serialize, Deserialize, GraphQLObject)]
#[serde(rename_all = "camelCase")]
pub struct Doc {
    pub id: Uuid,
    pub front: Front,
    pub content: String,
}

impl From<db::DocEntity> for Doc {
    fn from(entity: db::DocEntity) -> Self {
        let db::DocEntity {
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
            ..
        } = entity;

        Doc {
            id,
            front: Front {
                title,
                outline,
                author,
                tags,
                image,
                kind: DocKind::from(kind),
                genre: DocGenre::from(genre),
                updated_at,
            },
            content,
        }
    }
}

// use crate::state::{argon, jwt};
// use crate::fsm;

#[derive(Debug, Deserialize, Serialize, GraphQLObject)]
#[serde(rename_all = "camelCase")]
pub struct SingleDocResponseBody {
    pub doc: Option<Doc>,
}

impl From<Doc> for SingleDocResponseBody {
    fn from(doc: Doc) -> Self {
        Self { doc: Some(doc) }
    }
}

/// The response body for multiple documents
#[derive(Debug, Deserialize, Serialize, GraphQLObject)]
#[serde(rename_all = "camelCase")]
pub struct MultiDocsResponseBody {
    pub docs: Vec<Doc>,
    pub docs_count: i32,
}

impl From<Vec<Doc>> for MultiDocsResponseBody {
    fn from(docs: Vec<Doc>) -> Self {
        let docs_count = i32::try_from(docs.len()).unwrap();
        Self { docs, docs_count }
    }
}

// I haven't found a way to have struct that can be both GraphQLInputObject and GraphQLObject.
// I would have like to use Doc to create a new document, but it doesn't work. So this is
// the I don't want to think about it solution...
#[derive(Debug, Deserialize, Serialize, GraphQLInputObject)]
pub struct DocSpec {
    pub id: Uuid,
    pub title: String,
    pub outline: String,
    pub author: String,
    pub tags: Vec<String>,
    pub image: String,
    pub kind: DocKind,
    pub genre: DocGenre,
    pub content: String,
}

impl From<DocSpec> for db::DocEntity {
    fn from(spec: DocSpec) -> Self {
        let DocSpec {
            id,
            title,
            outline,
            author,
            tags,
            image,
            kind,
            genre,
            content,
            ..
        } = spec;

        db::DocEntity {
            id,
            title,
            outline,
            author,
            tags,
            image,
            kind: db::DocKind::from(kind),
            genre: db::DocGenre::from(genre),
            content,
            updated_at: Utc::now(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, GraphQLInputObject)]
pub struct DocumentRequestBody {
    pub doc: DocSpec,
}

/// The response body for a document creation.
#[derive(Debug, Deserialize, Serialize, GraphQLObject)]
#[serde(rename_all = "camelCase")]
pub struct DocumentCreationResponseBody {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
}

impl From<db::DocCreationAck> for DocumentCreationResponseBody {
    fn from(ack: db::DocCreationAck) -> Self {
        Self {
            id: ack.id,
            created_at: ack.created_at,
        }
    }
}

/// Retrieve all documents
pub async fn list_documents(context: &Context) -> Result<MultiDocsResponseBody, error::Error> {
    async move {
        let pool = &context.state.pool;

        let mut tx = pool
            .conn()
            .and_then(Connection::begin)
            .await
            .context(error::DBError {
                msg: "could not initiate transaction",
            })?;

        let entities = tx
            .get_all_documents()
            .await
            .context(error::DBProvideError {
                msg: "Could not get all them documents",
            })?;

        let documents = entities.into_iter().map(Doc::from).collect::<Vec<_>>();

        tx.commit().await.context(error::DBError {
            msg: "could not commit transaction",
        })?;

        Ok(MultiDocsResponseBody::from(documents))
    }
    .await
}

/// Retrieve a single document given its id
pub async fn find_document_by_id(
    context: &Context,
    id: Uuid,
) -> Result<SingleDocResponseBody, error::Error> {
    async move {
        let pool = &context.state.pool;

        let mut tx = pool
            .conn()
            .and_then(Connection::begin)
            .await
            .context(error::DBError {
                msg: "could not initiate transaction",
            })?;

        let entity = tx
            .get_document_by_id(id)
            .await
            .context(error::DBProvideError {
                msg: "Could not get document by id",
            });

        match entity {
            Err(err) => {
                info!(context.state.logger, "DB Provide Error: {:?}", err);
                Err(err)
            }
            Ok(entity) => {
                tx.commit().await.context(error::DBError {
                    msg: "could not commit transaction",
                })?;
                match entity {
                    None => Ok(SingleDocResponseBody { doc: None }),
                    Some(entity) => {
                        let doc = Doc::from(entity);
                        Ok(SingleDocResponseBody::from(doc))
                    }
                }
            }
        }
    }
    .await
}

/// Create a new document, or update it if it exists already
pub async fn create_or_update_document(
    doc_request: DocumentRequestBody,
    context: &Context,
) -> Result<DocumentCreationResponseBody, error::Error> {
    async move {
        let DocumentRequestBody { doc } = doc_request;
        let doc = db::DocEntity::from(doc);

        let pool = &context.state.pool;

        let mut tx = pool
            .conn()
            .and_then(Connection::begin)
            .await
            .context(error::DBError {
                msg: "could not initiate transaction",
            })?;

        let resp = ProvideData::create_or_update_document(&mut tx as &mut sqlx::PgConnection, &doc)
            .await
            .context(error::DBProvideError {
                msg: "Could not create or update document",
            })?;

        tx.commit().await.context(error::DBError {
            msg: "could not retrieve indexes",
        })?;

        Ok(DocumentCreationResponseBody::from(resp))
    }
    .await
}

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
use crate::db::model::ProvideJournal;
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
pub struct Author {
    pub fullname: String,
    pub resource: String,
}

impl From<db::AuthorEntity> for Author {
    fn from(entity: db::AuthorEntity) -> Self {
        let db::AuthorEntity {
            fullname, resource, ..
        } = entity;

        Author { fullname, resource }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, GraphQLObject)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    pub title: String,
    pub resource: String,
    pub author: Author,
}

impl From<db::ImageEntity> for Image {
    fn from(entity: db::ImageEntity) -> Self {
        let db::ImageEntity {
            title,
            author,
            resource,
            ..
        } = entity;

        Image {
            title,
            author: Author::from(author),
            resource,
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, GraphQLObject)]
#[serde(rename_all = "camelCase")]
pub struct Front {
    pub title: String,
    #[serde(rename = "abstract")]
    pub outline: String,
    pub author: Author,
    pub tags: Vec<String>,
    pub image: Image,
    #[serde(default = "default_kind")]
    pub kind: DocKind,
    #[serde(default = "default_genre")]
    pub genre: DocGenre,
    pub created_at: DateTime<Utc>,
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

#[derive(Debug, PartialEq, Serialize, Deserialize, GraphQLObject)]
#[serde(rename_all = "camelCase")]
pub struct ShortDoc {
    pub id: Uuid,
    pub front: Front,
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
            created_at,
            updated_at,
            ..
        } = entity;

        Doc {
            id,
            front: Front {
                title,
                outline,
                author: Author::from(author),
                tags,
                image: Image::from(image),
                kind: DocKind::from(kind),
                genre: DocGenre::from(genre),
                created_at,
                updated_at,
            },
            content,
        }
    }
}

impl From<db::ShortDocEntity> for ShortDoc {
    fn from(entity: db::ShortDocEntity) -> Self {
        let db::ShortDocEntity {
            id,
            title,
            outline,
            author,
            tags,
            image,
            kind,
            genre,
            created_at,
            updated_at,
            ..
        } = entity;

        ShortDoc {
            id,
            front: Front {
                title,
                outline,
                author: Author::from(author),
                tags,
                image: Image::from(image),
                kind: DocKind::from(kind),
                genre: DocGenre::from(genre),
                created_at,
                updated_at,
            },
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
    pub docs: Vec<ShortDoc>,
    pub docs_count: i32,
}

impl From<Vec<ShortDoc>> for MultiDocsResponseBody {
    fn from(docs: Vec<ShortDoc>) -> Self {
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
    pub author_fullname: String,
    pub author_resource: String,
    pub tags: Vec<String>,
    pub image_title: String,
    pub image_resource: String,
    pub image_author_fullname: String,
    pub image_author_resource: String,
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
            author_fullname,
            author_resource,
            tags,
            image_title,
            image_resource,
            image_author_fullname,
            image_author_resource,
            kind,
            genre,
            content,
            ..
        } = spec;

        let author = db::AuthorEntity {
            id: None,
            fullname: author_fullname,
            resource: author_resource,
        };

        let image_author = db::AuthorEntity {
            id: None,
            fullname: image_author_fullname,
            resource: image_author_resource,
        };

        let image = db::ImageEntity {
            id: None,
            title: image_title,
            author: image_author,
            resource: image_resource,
        };

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
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, GraphQLInputObject)]
pub struct DocumentRequestBody {
    pub doc: DocSpec,
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

        let documents = entities.into_iter().map(ShortDoc::from).collect::<Vec<_>>();

        tx.commit().await.context(error::DBError {
            msg: "could not commit transaction",
        })?;

        Ok(MultiDocsResponseBody::from(documents))
    }
    .await
}

/// search all documents for matching query
pub async fn list_documents_by_query(
    context: &Context,
    query: &str,
) -> Result<MultiDocsResponseBody, error::Error> {
    async move {
        let pool = &context.state.pool;

        let mut tx = pool
            .conn()
            .and_then(Connection::begin)
            .await
            .context(error::DBError {
                msg: "could not initiate transaction",
            })?;

        let entities =
            tx.get_all_documents_by_query(query)
                .await
                .context(error::DBProvideError {
                    msg: "Could not get all them documents",
                })?;

        let documents = entities.into_iter().map(ShortDoc::from).collect::<Vec<_>>();

        tx.commit().await.context(error::DBError {
            msg: "could not commit transaction",
        })?;

        Ok(MultiDocsResponseBody::from(documents))
    }
    .await
}

/// search all documents for matching tag
pub async fn list_documents_by_tag(
    context: &Context,
    tag: &str,
) -> Result<MultiDocsResponseBody, error::Error> {
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
            .get_all_documents_by_tag(tag)
            .await
            .context(error::DBProvideError {
                msg: "Could not get all them documents",
            })?;

        let documents = entities.into_iter().map(ShortDoc::from).collect::<Vec<_>>();

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
) -> Result<SingleDocResponseBody, error::Error> {
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

        let resp =
            ProvideJournal::create_or_update_document(&mut tx as &mut sqlx::PgConnection, &doc)
                .await
                .context(error::DBProvideError {
                    msg: "Could not create or update document",
                })?;

        tx.commit().await.context(error::DBError {
            msg: "could not retrieve indexes",
        })?;

        let doc = Doc::from(resp);
        Ok(SingleDocResponseBody::from(doc))
    }
    .await
}

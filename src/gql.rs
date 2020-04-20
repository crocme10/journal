use super::model::{Doc, DocKind, DocSummary};
use juniper::{FieldResult, GraphQLObject, GraphQLType};
use log::{debug, info, error};
use sqlx::postgres::{PgPool, PgQueryAs};
use uuid::Uuid;

#[derive(Debug)]
pub struct Context {
    pub pool: PgPool,
}

impl juniper::Context for Context {}

#[derive(GraphQLObject, Debug)]
pub struct DocListResp {
    pub ok: bool,
    pub error: Option<String>,
    pub docs: Option<Vec<DocSummary>>,
}

#[derive(GraphQLObject, Debug)]
pub struct DocResp {
    pub ok: bool,
    pub error: Option<String>,
    pub doc: Option<Doc>,
}

pub struct Query;

#[juniper::graphql_object(Context = Context)]
impl Query {
    async fn docs(&self, context: &Context) -> FieldResult<DocListResp> {
        debug!("Querying Documents List");

        // Search all documents with the kind = 'doc'
        match sqlx::query_as("SELECT * FROM document_list($1)")
            .bind(DocKind::Doc)
            .fetch_all(&context.pool)
            .await
        {
            Ok(rows) => {
                let docs: Vec<DocSummary> = rows.into();
                Ok(DocListResp {
                    ok: true,
                    error: None,
                    docs: Some(docs),
                })
            }
            Err(err) => {
                error!("Error retrieving document list: {}", err);
                Ok(DocListResp {
                    ok: false,
                    error: Some(format!("Document List Error: {}", err)),
                    docs: None,
                })
            }
        }
    }

    async fn doc(&self, id: Uuid, context: &Context) -> FieldResult<DocResp> {
        info!("Querying Document {}", id);

        let res: Result<Doc, sqlx::Error> = sqlx::query_as("SELECT * FROM document_details($1)")
            .bind::<Uuid>(id)
            .fetch_one(&context.pool)
            .await;

        match res {
            Ok(doc) => Ok(DocResp {
                ok: true,
                error: None,
                doc: Some(doc),
            }),
            Err(err) => {
                error!("Error retrieving Document Detail: {}", err);
                Ok(DocResp {
                    ok: false,
                    error: Some(format!("Document Details Error: {}", err)),
                    doc: None,
                })
            }
        }
    }

    async fn doc_search(&self, search: String, context: &Context) -> FieldResult<DocListResp> {
        debug!("Querying Documents Search {}", search);

        // Search all documents with the kind = 'doc'
        match sqlx::query_as("SELECT * FROM document_search($1)")
            .bind(&search)
            .fetch_all(&context.pool)
            .await
        {
            Ok(rows) => {
                let docs: Vec<DocSummary> = rows.into();
                Ok(DocListResp {
                    ok: true,
                    error: None,
                    docs: Some(docs),
                })
            }
            Err(err) => {
                error!("Error searching for documents: {}", err);
                Ok(DocListResp {
                    ok: false,
                    error: Some(format!("Document Search Error: {}", err)),
                    docs: None,
                })
            }
        }
    }

    async fn tag_search(&self, tag: String, context: &Context) -> FieldResult<DocListResp> {
        debug!("Querying Tag Search {}", tag);

        // Search all documents with the kind = 'doc'
        match sqlx::query_as("SELECT * FROM document_tag($1, $2)")
            .bind(DocKind::Doc)
            .bind(&tag)
            .fetch_all(&context.pool)
            .await
        {
            Ok(rows) => {
                let docs: Vec<DocSummary> = rows.into();
                Ok(DocListResp {
                    ok: true,
                    error: None,
                    docs: Some(docs),
                })
            }
            Err(err) => {
                error!("Error searching for documents by tag: {}", err);
                Ok(DocListResp {
                    ok: false,
                    error: Some(format!("Document Search Error: {}", err)),
                    docs: None,
                })
            }
        }
    }
}

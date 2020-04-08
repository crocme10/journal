use super::model::{Doc, DocSummary};
use juniper::{FieldResult, GraphQLObject, GraphQLType};
use log::{debug, info};
use tokio_postgres::Client;
use uuid::Uuid;

#[derive(Debug)]
pub struct Context {
    pub client: Client,
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

#[juniper::graphql_object( Context = Context)]
impl Query {
    async fn docs(&self, context: &Context) -> FieldResult<DocListResp> {
        debug!("Querying Documents List");

        // Search all documents with the kind = 'doc'
        match context
            .client
            .query("SELECT * FROM document_list('doc')", &[])
            .await
        {
            Ok(rows) => {
                let docs: Vec<DocSummary> = rows.iter().map(|r| r.into()).collect();
                Ok(DocListResp {
                    ok: true,
                    error: None,
                    docs: Some(docs),
                })
            }
            Err(err) => Ok(DocListResp {
                ok: false,
                error: Some(format!("Document List Error: {}", err)),
                docs: None,
            }),
        }
    }

    async fn doc(&self, id: Uuid, context: &Context) -> FieldResult<DocResp> {
        info!("Querying Document {}", id);

        match context
            .client
            .query("SELECT * FROM document_details($1)", &[&id])
            .await
        {
            Ok(rows) => {
                let doc: Doc = rows.get(0).unwrap().into();
                Ok(DocResp {
                    ok: true,
                    error: None,
                    doc: Some(doc),
                })
            }
            Err(err) => {
                debug!("Document Detail Error: {}", err);
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
        match context
            .client
            .query("SELECT * FROM document_search($1)", &[&search])
            .await
        {
            Ok(rows) => {
                let docs: Vec<DocSummary> = rows.iter().map(|r| r.into()).collect();
                Ok(DocListResp {
                    ok: true,
                    error: None,
                    docs: Some(docs),
                })
            }
            Err(err) => Ok(DocListResp {
                ok: false,
                error: Some(format!("Document Search Error: {}", err)),
                docs: None,
            }),
        }
    }

    async fn tag_search(&self, tag: String, context: &Context) -> FieldResult<DocListResp> {
        debug!("Querying Tag Search {}", tag);

        // Search all documents with the kind = 'doc'
        match context
            .client
            .query("SELECT * FROM document_tag('doc', $1)", &[&tag])
            .await
        {
            Ok(rows) => {
                let docs: Vec<DocSummary> = rows.iter().map(|r| r.into()).collect();
                Ok(DocListResp {
                    ok: true,
                    error: None,
                    docs: Some(docs),
                })
            }
            Err(err) => Ok(DocListResp {
                ok: false,
                error: Some(format!("Document Search Error: {}", err)),
                docs: None,
            }),
        }
    }
}

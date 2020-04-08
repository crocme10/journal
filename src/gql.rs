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
            Err(_err) => Ok(DocListResp {
                ok: false,
                error: Some("Not existing user".to_string()),
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
                debug!("Got document: {:?}", doc);
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
                    error: Some("Not existing user".to_string()),
                    doc: None,
                })
            }
        }
    }
}

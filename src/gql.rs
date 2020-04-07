use super::model::DocSummary;
//use bb8_postgres::{bb8::Pool, PostgresConnectionManager};
use juniper::{FieldResult, GraphQLObject, GraphQLType};
use log::info;
use tokio_postgres::Client;
// use uuid::Uuid;

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

pub struct Query;

#[juniper::graphql_object( Context = Context)]
impl Query {
    async fn docs(&self, context: &Context) -> FieldResult<DocListResp> {
        info!("Querying Documents List");

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
}

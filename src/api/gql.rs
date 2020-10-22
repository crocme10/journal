use juniper::{EmptySubscription, FieldResult, IntoFieldError, RootNode};
use uuid::Uuid;

use super::model;
use crate::error;
use crate::state::State;

#[derive(Debug, Clone)]
pub struct Context {
    pub state: State,
}

impl juniper::Context for Context {}

pub struct Query;

#[juniper::graphql_object(
    Context = Context
)]
impl Query {
    /// Returns a list of documents
    async fn documents(&self, context: &Context) -> FieldResult<model::MultiDocsResponseBody> {
        model::list_documents(context)
            .await
            .map_err(IntoFieldError::into_field_error)
            .into()
    }

    /// Find a document by its id
    async fn find_document_by_id(
        &self,
        id: Uuid,
        context: &Context,
    ) -> FieldResult<model::SingleDocResponseBody> {
        model::find_document_by_id(context, id)
            .await
            .map_err(IntoFieldError::into_field_error)
    }
}

pub struct Mutation;

#[juniper::graphql_object(
    Context = Context
)]
impl Mutation {
    async fn create_or_update_document(
        &self,
        doc: model::DocumentRequestBody,
        context: &Context,
    ) -> FieldResult<model::DocumentCreationResponseBody> {
        model::create_or_update_document(doc, context)
            .await
            .map_err(IntoFieldError::into_field_error)
    }
}

type Schema = RootNode<'static, Query, Mutation, EmptySubscription<Context>>;

pub fn schema() -> Schema {
    Schema::new(Query, Mutation, EmptySubscription::new())
}

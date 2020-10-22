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
    async fn add_document(
        &self,
        _doc: model::DocumentUpdateRequestBody,
        _context: &Context,
    ) -> FieldResult<model::SingleDocResponseBody> {
        // users::add_user(user, context)
        //     .await
        //     .map_err(IntoFieldError::into_field_error)
        Err(IntoFieldError::into_field_error(error::Error::MiscError {
            msg: String::from("not implemented"),
        }))
    }
}

type Schema = RootNode<'static, Query, Mutation, EmptySubscription<Context>>;

pub fn schema() -> Schema {
    Schema::new(Query, Mutation, EmptySubscription::new())
}

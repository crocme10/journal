use juniper::{EmptySubscription, FieldResult, IntoFieldError, RootNode};
use slog::info;
use uuid::Uuid;

use crate::api::model;
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
    async fn list_documents(&self, context: &Context) -> FieldResult<model::MultiDocsResponseBody> {
        info!(context.state.logger, "Request for documents");
        model::list_documents(context, model::DocKind::Doc)
            .await
            .map_err(IntoFieldError::into_field_error)
    }

    /// Returns a list of posts
    async fn list_posts(&self, context: &Context) -> FieldResult<model::MultiDocsResponseBody> {
        info!(context.state.logger, "Request for posts");
        model::list_documents(context, model::DocKind::Post)
            .await
            .map_err(IntoFieldError::into_field_error)
    }

    /// Find a document by its id
    async fn find_document_by_id(
        &self,
        id: Uuid,
        context: &Context,
    ) -> FieldResult<model::SingleDocResponseBody> {
        info!(context.state.logger, "Request for document with id {}", id);
        model::find_document_by_id(context, id)
            .await
            .map_err(IntoFieldError::into_field_error)
    }

    /// Returns a list of documents using full text search.
    async fn list_documents_by_query(
        &self,
        query: String,
        context: &Context,
    ) -> FieldResult<model::MultiDocsResponseBody> {
        info!(
            context.state.logger,
            "Request for documents search using query {}", query
        );
        model::list_documents_by_query(context, query.as_str())
            .await
            .map_err(IntoFieldError::into_field_error)
    }

    /// Returns a list of documents using full text search.
    async fn list_documents_by_tag(
        &self,
        tag: String,
        context: &Context,
    ) -> FieldResult<model::MultiDocsResponseBody> {
        info!(
            context.state.logger,
            "Request for documents search using tag {}", tag
        );
        model::list_documents_by_tag(context, tag.as_str())
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
    ) -> FieldResult<model::SingleDocResponseBody> {
        info!(
            context.state.logger,
            "Request for document update with id {}", doc.doc.id
        );
        model::create_or_update_document(doc, context)
            .await
            .map_err(IntoFieldError::into_field_error)
    }
}

type Schema = RootNode<'static, Query, Mutation, EmptySubscription<Context>>;

pub fn schema() -> Schema {
    Schema::new(Query, Mutation, EmptySubscription::new())
}

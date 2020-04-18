use chrono::prelude::*;
use juniper::{GraphQLEnum, GraphQLObject};
use serde::{Deserialize, Serialize};
use sqlx::{
    postgres::PgRow,
    row::{FromRow, Row},
};
use uuid::Uuid;

#[derive(Debug, sqlx::Type, PartialEq, Serialize, Deserialize, GraphQLEnum)]
#[sqlx(rename = "kind")]
#[sqlx(rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum DocKind {
    Doc,
    Post,
}

#[derive(Debug, sqlx::Type, PartialEq, Serialize, Deserialize, GraphQLEnum)]
#[sqlx(rename = "genre")]
#[sqlx(rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum DocGenre {
    Tutorial,
    Howto,
    Background,
    Reference,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, GraphQLObject)]
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
}

#[derive(Debug, PartialEq, Serialize, Deserialize, GraphQLObject)]
pub struct Doc {
    pub front: Front,
    pub id: Uuid,
    pub updated_at: DateTime<Utc>,
    pub content: String,
}

pub fn default_kind() -> DocKind {
    DocKind::Doc
}

pub fn default_genre() -> DocGenre {
    DocGenre::Tutorial
}

impl<'c> FromRow<'c, PgRow<'c>> for Doc {
    fn from_row(row: &PgRow<'c>) -> Result<Self, sqlx::Error> {
        Ok(Doc {
            id: row.get(0),
            front: Front {
                title: row.get(1),
                outline: row.get(2),
                author: row.get(3),
                tags: row.get(4),
                image: row.get(5),
                kind: row.get(6),
                genre: row.get(7),
            },
            updated_at: row.get(8),
            content: row.get(9),
        })
    }
}

#[derive(Debug, GraphQLObject)]
pub struct DocSummary {
    pub front: Front,
    pub id: Uuid,
    pub updated_at: DateTime<Utc>,
}

impl<'c> FromRow<'c, PgRow<'c>> for DocSummary {
    fn from_row(row: &PgRow<'c>) -> Result<Self, sqlx::Error> {
        Ok(DocSummary {
            id: row.get(0),
            front: Front {
                title: row.get(1),
                outline: row.get(2),
                author: row.get(3),
                tags: row.get(4),
                image: row.get(5),
                kind: row.get(6),
                genre: row.get(7),
            },
            updated_at: row.get(8),
        })
    }
}

use chrono::prelude::*;
use juniper::{GraphQLEnum, GraphQLObject};
use postgres_types::{FromSql, ToSql};
use serde::{Deserialize, Serialize};
use tokio_postgres::row::Row;
use uuid::Uuid;

#[derive(Debug, FromSql, ToSql, PartialEq, Serialize, Deserialize, GraphQLEnum)]
#[postgres(name = "kind")]
#[serde(rename_all = "lowercase")]
pub enum DocKind {
    #[postgres(name = "doc")]
    Doc,
    #[postgres(name = "post")]
    Post,
}

#[derive(Debug, FromSql, ToSql, PartialEq, Serialize, Deserialize, GraphQLEnum)]
#[postgres(name = "genre")]
#[serde(rename_all = "lowercase")]
pub enum DocGenre {
    #[postgres(name = "tutorial")]
    Tutorial,
    #[postgres(name = "howto")]
    Howto,
    #[postgres(name = "background")]
    Background,
    #[postgres(name = "reference")]
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

impl From<Row> for Doc {
    fn from(row: Row) -> Self {
        Doc {
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
        }
    }
}

impl From<&Row> for Doc {
    fn from(row: &Row) -> Self {
        Doc {
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
        }
    }
}
#[derive(Debug, GraphQLObject)]
pub struct DocSummary {
    pub front: Front,
    pub id: Uuid,
    pub updated_at: DateTime<Utc>,
}

impl From<Row> for DocSummary {
    fn from(row: Row) -> Self {
        DocSummary {
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
        }
    }
}

impl From<&Row> for DocSummary {
    fn from(row: &Row) -> Self {
        DocSummary {
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
        }
    }
}

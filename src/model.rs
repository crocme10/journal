use postgres_types::ToSql;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, ToSql, PartialEq, Serialize, Deserialize)]
#[postgres(name = "doc_kind")]
pub enum DocKind {
    #[postgres(name = "doc")]
    Doc,
    #[postgres(name = "post")]
    Post,
}

#[derive(Debug, ToSql, PartialEq, Serialize, Deserialize)]
#[postgres(name = "doc_genre")]
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

#[derive(Debug, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Doc {
    pub front: Front,
    pub id: Uuid,
    pub content: String,
}

pub fn default_kind() -> DocKind {
    DocKind::Doc
}

pub fn default_genre() -> DocGenre {
    DocGenre::Tutorial
}

use chrono::NaiveDateTime;
use diesel::{Queryable, Insertable};
use serde::{Deserialize, Serialize};

use crate::schema::{posts, users};

#[derive(Queryable, Serialize)]
pub struct User {
    pub id: i64,
    pub profile: String,
    pub email: String,
    pub firstname: String,
    pub lastname: String,
    pub ph: String,
    pub password: String,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}

#[derive(Insertable, Deserialize)]
#[diesel(table_name = users)]
pub struct NewUser {
    pub profile: String,
    pub email: String,
    pub firstname: String,
    pub lastname: String,
    pub ph: String,
    pub password: String,
}

#[derive(Insertable)]
#[diesel(table_name = posts)]
pub struct NewPost {
    pub userid: i64,
    pub name: String,
    pub description: String,
    pub imgs: Vec<Option<String>>,
}

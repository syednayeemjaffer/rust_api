use chrono::NaiveDateTime;
use diesel::{Queryable, Insertable};
use serde::{Deserialize, Serialize};

use crate::schema::users;

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

use chrono::NaiveDateTime;
use diesel::{Queryable, Insertable};
use serde::{Deserialize, Serialize};

use crate::schema::{posts, users};

// USER MODELS 

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

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize, Deserialize,Clone)]
pub struct Claims {
    pub id: i64,
    pub email: String,
    pub firstname: String,
    pub lastname: String,
    pub exp: usize,
}

#[derive(Serialize, Queryable)]
pub struct UserData {
    pub id: i64,
    pub firstname: String,
    pub lastname: String,
    pub email: String,
    pub ph: String,
    pub profile: String,
}

#[derive(Deserialize)]
pub struct ChangePasswordForm {
    pub old_password: String,
    pub new_password: String,
}

// POST MODELS 

#[derive(Queryable, Serialize)]
pub struct Post {
    pub id: i32,
    pub userid: i64,
    pub name: String,
    pub description: String,
    pub imgs: Vec<Option<String>>,
    pub created_at: Option<NaiveDateTime>,
}

#[derive(Insertable)]
#[diesel(table_name = posts)]
pub struct NewPost {
    pub userid: i64,
    pub name: String,
    pub description: String,
    pub imgs: Vec<Option<String>>,
}

#[derive(Serialize, Queryable)]
pub struct PostData {
    pub id: i32,
    pub userid: i64,
    pub name: String,
    pub description: String,
    pub imgs: Vec<Option<String>>,
    pub created_at: Option<NaiveDateTime>,
}

#[derive(Serialize)]
pub struct PostWithUser {
    pub id: i32,
    pub user_id: i64,
    pub firstname: String,
    pub lastname: String,
    pub email: String,
    pub profile: String,
    pub name: String,
    pub imgs: Vec<String>,
    pub description: String,
    pub created_at: Option<NaiveDateTime>,
}

#[derive(Deserialize)]
pub struct UpdatePostFields {
    pub name: Option<String>,
    pub description: Option<String>,
}
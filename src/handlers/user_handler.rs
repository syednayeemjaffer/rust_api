use actix_multipart::Multipart;
use actix_web::{HttpResponse, Responder, post, web};
use bcrypt::{DEFAULT_COST, hash};
use diesel::prelude::*;
use futures_util::TryStreamExt as _;

use crate::{
    db::Pool,
    models::user::{NewUser, User},
    schema::users::dsl::*,
    utils::{file_upload::save_profile_image, validation::Validator},
};

#[post("/register")]
pub async fn register_user(pool: web::Data<Pool>, mut payload: Multipart) -> impl Responder {
    // ---- Collect form-data ----
    let mut profile_filename: Option<String> = None;
    let mut email_field = String::new();
    let mut firstname_field = String::new();
    let mut lastname_field = String::new();
    let mut phone_field = String::new();
    let mut password_field = String::new();
    let mut image_bytes: Option<Vec<u8>> = None;

    while let Ok(Some(mut field)) = payload.try_next().await {
        let name = field.name().to_string();

        if name == "profile" {
            let content_disposition = field.content_disposition();
            let filename = content_disposition
                .get_filename()
                .map(|f| f.to_string())
                .unwrap_or_else(|| "unknown.jpg".to_string());

            if let Err(e) = Validator::validate_image_type(&filename) {
                return HttpResponse::BadRequest()
                    .json(serde_json::json!({ "status": false, "message": e }));
            }

            let mut bytes = Vec::new();
            while let Some(chunk) = field.try_next().await.unwrap() {
                bytes.extend_from_slice(&chunk);
            }

            if bytes.len() > 3 * 1024 * 1024 {
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "status": false, "message": "File size should be less than 3MB"
                }));
            }

            image_bytes = Some(bytes);
            profile_filename = Some(filename);
        } else {
            let mut data = Vec::new();
            while let Some(chunk) = field.try_next().await.unwrap() {
                data.extend_from_slice(&chunk);
            }
            let value = String::from_utf8_lossy(&data).to_string();

            match name.as_str() {
                "email" => email_field = value,
                "firstname" => firstname_field = value,
                "lastname" => lastname_field = value,
                "ph" => phone_field = value,
                "password" => password_field = value,
                _ => {}
            }
        }
    }

    // ---- Validations ----
    if email_field.is_empty()
        || firstname_field.is_empty()
        || lastname_field.is_empty()
        || phone_field.is_empty()
        || password_field.is_empty()
    {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": false, "message": "All fields are required"
        }));
    }

    if let Err(e) = Validator::validate_email(&email_field) {
        return HttpResponse::BadRequest()
            .json(serde_json::json!({ "status": false, "message": e }));
    }
    if let Err(e) = Validator::validate_firstname(&firstname_field) {
        return HttpResponse::BadRequest()
            .json(serde_json::json!({ "status": false, "message": e }));
    }
    if let Err(e) = Validator::validate_lastname(&lastname_field) {
        return HttpResponse::BadRequest()
            .json(serde_json::json!({ "status": false, "message": e }));
    }
    if let Err(e) = Validator::validate_phone(&phone_field) {
        return HttpResponse::BadRequest()
            .json(serde_json::json!({ "status": false, "message": e }));
    }
    if let Err(e) = Validator::validate_password(&password_field) {
        return HttpResponse::BadRequest()
            .json(serde_json::json!({ "status": false, "message": e }));
    }

    // ---- Database checks ----
    let mut conn = pool.get().expect("Couldn't get DB connection from pool");

    let existing_user = users
        .filter(email.eq(&email_field))
        .first::<User>(&mut conn)
        .optional()
        .expect("Failed to query user");

    if existing_user.is_some() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": false, "message": "Email already exists"
        }));
    }

    // ---- Save image last ----
    if image_bytes.is_none() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": false, "message": "Profile image is required"
        }));
    }

    let saved_filename = match save_profile_image(image_bytes.unwrap(), &profile_filename.unwrap())
    {
        Ok(name) => name,
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "status": false, "message": e
            }));
        }
    };

    // ---- Hash & insert ----
    let hashed_pwd = hash(&password_field, DEFAULT_COST).unwrap();

    let new_user = NewUser {
        profile: saved_filename,
        email: email_field,
        firstname: firstname_field,
        lastname: lastname_field,
        ph: phone_field,
        password: hashed_pwd,
    };

    diesel::insert_into(users)
        .values(&new_user)
        .execute(&mut conn)
        .expect("Error inserting new user");

    HttpResponse::Created().json(serde_json::json!({
        "status": true,
        "message": "User created successfully"
    }))
}

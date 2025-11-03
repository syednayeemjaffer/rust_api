use actix_multipart::Multipart;
use actix_web::{HttpResponse, Responder, web};
use bcrypt::{DEFAULT_COST, hash, verify};
use diesel::prelude::*;
use futures_util::TryStreamExt as _;
use jsonwebtoken::{EncodingKey, Header, encode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;

use crate::{
    db::Pool,
    models::user::{NewUser, User},
    schema::users,
    utils::{file_upload::save_profile_image, validation::Validator},
};

#[derive(Serialize, Queryable)]
pub struct UserData {
    pub id: i64,
    pub firstname: String,
    pub lastname: String,
    pub email: String,
    pub ph: String,
    pub profile: String,
}

#[derive(Serialize)]
struct UsersResponse {
    status: bool,
    totalUsers: i64,
    users: Vec<UserData>,
}

#[derive(Deserialize)]
pub struct ChangePasswordForm {
    pub old_password: String,
    pub new_password: String,
}

pub async fn register_user(pool: web::Data<Pool>, mut payload: Multipart) -> impl Responder {
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
                .unwrap_or("unknown.jpg")
                .to_string();

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
                    "status": false,
                    "message": "File size should be less than 3MB"
                }));
            }

            image_bytes = Some(bytes);
            profile_filename = Some(filename);
        } else {
            let mut data: Vec<u8> = Vec::new();
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

    if email_field.is_empty()
        || firstname_field.is_empty()
        || lastname_field.is_empty()
        || phone_field.is_empty()
        || password_field.is_empty()
    {
        return HttpResponse::BadRequest()
            .json(serde_json::json!({"status": false, "message": "All fields are required"}));
    }

    let mut conn = pool.get().expect("DB connection error");

    let exists = users::table
        .filter(users::email.eq(&email_field))
        .first::<User>(&mut conn)
        .optional()
        .expect("DB query failed");

    if exists.is_some() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": false,
            "message": "Email already exists"
        }));
    }

    let saved_filename = match save_profile_image(image_bytes.unwrap(), &profile_filename.unwrap())
    {
        Ok(name) => name,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .json(serde_json::json!({"status": false, "message": e}));
        }
    };

    let hashed_pwd = hash(&password_field, DEFAULT_COST).unwrap();

    let new_user = NewUser {
        profile: saved_filename,
        email: email_field,
        firstname: firstname_field,
        lastname: lastname_field,
        ph: phone_field,
        password: hashed_pwd,
    };

    diesel::insert_into(users::table)
        .values(&new_user)
        .execute(&mut conn)
        .expect("Insert failed");

    HttpResponse::Created().json(serde_json::json!({
        "status": true,
        "message": "User created successfully"
    }))
}

// LOGIN USER 
#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Claims {
    pub id: i64,
    pub email: String,
    pub firstname: String,
    pub lastname: String,
    pub exp: usize,
}

pub async fn login_user(pool: web::Data<Pool>, body: web::Json<LoginRequest>) -> impl Responder {
    let mut conn = pool.get().expect("DB connection error");
    let email_field = body.email.trim().to_string();
    let password_field = body.password.trim().to_string();

    let user = users::table
        .filter(users::email.eq(&email_field))
        .first::<User>(&mut conn)
        .optional()
        .expect("DB query failed");

    let user = match user {
        Some(u) => u,
        None => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "status": false,
                "message": "User not found"
            }));
        }
    };

    if !verify(&password_field, &user.password).unwrap_or(false) {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": false,
            "message": "Incorrect password"
        }));
    }

    let secret = env::var("JWT_SECRET").unwrap_or_else(|_| "supersecretkey".into());
    let exp = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::days(1))
        .unwrap()
        .timestamp() as usize;

    let claims = Claims {
        id: user.id,
        email: user.email.clone(),
        firstname: user.firstname.clone(),
        lastname: user.lastname.clone(),
        exp,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap();

    HttpResponse::Ok().json(serde_json::json!({
        "status": true,
        "message": "Login successful",
        "token": token
    }))
}

pub async fn get_all_users(
    pool: web::Data<Pool>,
    query: web::Query<HashMap<String, String>>,
) -> impl Responder {
    let page = query
        .get("page")
        .and_then(|p| p.parse::<i64>().ok())
        .unwrap_or(1);
    let limit = query
        .get("limit")
        .and_then(|l| l.parse::<i64>().ok())
        .unwrap_or(3);
    let offset = (page - 1) * limit;

    let mut conn = pool.get().expect("DB connection error");

    let total_users: i64 = users::table.count().get_result(&mut conn).unwrap_or(0);

    let user_list = users::table
        .select((
            users::id,
            users::firstname,
            users::lastname,
            users::email,
            users::ph,
            users::profile,
        ))
        .limit(limit)
        .offset(offset)
        .load::<UserData>(&mut conn)
        .unwrap_or_else(|_| vec![]);

    HttpResponse::Ok().json(UsersResponse {
        status: true,
        totalUsers: total_users,
        users: user_list,
    })
}

pub async fn get_user_by_id(pool: web::Data<Pool>, path: web::Path<i64>) -> impl Responder {
    let user_id = path.into_inner();
    let mut conn = pool.get().expect("DB connection error");

    let user_result = users::table
        .select((
            users::id,
            users::firstname,
            users::lastname,
            users::email,
            users::ph,
            users::profile,
        ))
        .filter(users::id.eq(user_id))
        .first::<UserData>(&mut conn)
        .optional();

    match user_result {
        Ok(Some(user)) => HttpResponse::Ok().json(serde_json::json!({
            "status": true,
            "user": user
        })),
        Ok(None) => HttpResponse::BadRequest().json(serde_json::json!({
            "status": false,
            "message": "User not found"
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "status": false,
            "message": e.to_string()
        })),
    }
}

pub async fn update_user(
    pool: web::Data<Pool>,
    path: web::Path<i64>,
    mut payload: Multipart,
) -> impl Responder {
    use crate::schema::users::dsl::*;

    let user_id = path.into_inner();
    let mut conn = pool.get().expect("DB connection error");

    let existing_user = users
        .filter(id.eq(user_id))
        .first::<User>(&mut conn)
        .optional()
        .expect("DB query failed");

    if existing_user.is_none() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": false,
            "message": "User ID not found"
        }));
    }
    let mut user = existing_user.unwrap();
    let mut image_bytes: Option<Vec<u8>> = None;
    let mut uploaded_filename: Option<String> = None;

    while let Ok(Some(mut field)) = payload.try_next().await {
        let name = field.name().to_string();
        if name == "profile" {
            let cd = field.content_disposition();
            let filename = cd.get_filename().unwrap_or("unknown.jpg").to_string();

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
                    "status": false,
                    "message": "File size should be less than 3MB"
                }));
            }

            image_bytes = Some(bytes);
            uploaded_filename = Some(filename);
        } else {
            let mut data = Vec::new();
            while let Some(chunk) = field.try_next().await.unwrap() {
                data.extend_from_slice(&chunk);
            }
            let value = String::from_utf8_lossy(&data).to_string().trim().to_string();
            match name.as_str() {
                "firstname" => user.firstname = value,
                "lastname" => user.lastname = value,
                "email" => user.email = value,
                "ph" => user.ph = value,
                "password" => user.password = value,
                _ => {}
            }
        }
    }

    if let Err(e) = Validator::validate_firstname(&user.firstname) {
        return HttpResponse::BadRequest().json(serde_json::json!({"status": false, "message": e}));
    }
    if let Err(e) = Validator::validate_lastname(&user.lastname) {
        return HttpResponse::BadRequest().json(serde_json::json!({"status": false, "message": e}));
    }
    if let Err(e) = Validator::validate_phone(&user.ph) {
        return HttpResponse::BadRequest().json(serde_json::json!({"status": false, "message": e}));
    }
    if let Err(e) = Validator::validate_email(&user.email) {
        return HttpResponse::BadRequest().json(serde_json::json!({"status": false, "message": e}));
    }

    let email_exists = users
        .filter(email.eq(&user.email))
        .filter(id.ne(user_id))
        .first::<User>(&mut conn)
        .optional()
        .expect("DB check failed");
    if email_exists.is_some() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": false,
            "message": "Email already in use by another user"
        }));
    }

    // Handle the profile image update
    if let Some(bytes) = image_bytes {
        let saved_name = match save_profile_image(bytes, &uploaded_filename.unwrap()) {
            Ok(n) => n,
            Err(e) => {
                return HttpResponse::InternalServerError()
                    .json(serde_json::json!({"status": false, "message": e}));
            }
        };

        // Delete old profile file if exists
        let old_profile_path = format!("files/usersProfiles/{}", user.profile);
        if std::fs::metadata(&old_profile_path).is_ok() {
            let _ = std::fs::remove_file(&old_profile_path);
        }

        user.profile = saved_name;
    }

    // Hash password if changed
    if Validator::validate_password(&user.password).is_ok() {
        user.password = hash(&user.password, DEFAULT_COST).unwrap();
    }

    // Perform update
    let updated_rows = diesel::update(users.filter(id.eq(user_id)))
        .set((
            firstname.eq(&user.firstname),
            lastname.eq(&user.lastname),
            email.eq(&user.email),
            ph.eq(&user.ph),
            password.eq(&user.password),
            profile.eq(&user.profile),
        ))
        .execute(&mut conn)
        .expect("Failed to update user");

    if updated_rows == 0 {
        return HttpResponse::NotFound()
            .json(serde_json::json!({ "status": false, "message": "User not found" }));
    }

    HttpResponse::Ok().json(serde_json::json!({
        "status": true,
        "message": "User updated successfully"
    }))
}



pub async fn change_password(
    pool: web::Data<Pool>,
    path: web::Path<i64>,
    form: web::Json<ChangePasswordForm>,
) -> impl Responder {
    let user_id = path.into_inner();
    let old_password = &form.old_password;
    let new_password = &form.new_password;

    // Validate presence
    if old_password.is_empty() || new_password.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": false,
            "message": "All fields required"
        }));
    }

    // Validate new_password with your Regex validation in Validator
    if let Err(e) = Validator::validate_password(new_password) {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": false,
            "message": e,
        }));
    }

    let mut conn = pool.get().expect("DB connection error");

    // Fetch hashed password from DB
    let result = users::table
        .filter(users::id.eq(user_id))
        .select(users::password)
        .first::<String>(&mut conn)
        .optional();

    let hashed_password = match result {
        Ok(Some(pwd)) => pwd,
        Ok(None) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "status": false,
                "message": "User not found"
            }));
        }
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "status": false,
                "message": e.to_string()
            }));
        }
    };

    // Check old password matches
    if !verify(old_password, &hashed_password).unwrap_or(false) {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": false,
            "message": "Old password is incorrect"
        }));
    }

    // New password should not be same as old password
    if verify(new_password, &hashed_password).unwrap_or(false) {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": false,
            "message": "New password cannot be same as old password"
        }));
    }

    // Hash new password
    let new_hashed = match hash(new_password, DEFAULT_COST) {
        Ok(h) => h,
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "status": false,
                "message": format!("Hashing error: {}", e)
            }));
        }
    };

    // Update password in DB
    let updated = diesel::update(users::table.filter(users::id.eq(user_id)))
        .set(users::password.eq(new_hashed))
        .execute(&mut conn);

    match updated {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({
            "status": true,
            "message": "Password changed successfully"
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "status": false,
            "message": "Failed to update password",
            "error": e.to_string()
        })),
    }
}


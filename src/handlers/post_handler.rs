use actix_multipart::Multipart;
use actix_web::{HttpMessage, HttpRequest, HttpResponse, Responder, web};
use diesel::prelude::*;
use futures_util::TryStreamExt as _;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

use crate::{
    db::Pool,
    schema::{posts, users},
    utils::auth::Claims,
    utils::{img_upload::save_multiple_images, validation::Validator},
};

#[derive(Serialize)]
struct PostResponse {
    status: bool,
    message: String,
    post: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct PostsListResponse {
    status: bool,
    posts: Vec<serde_json::Value>,
    total_post: i64,
}

#[derive(Deserialize)]
pub struct DeleteImgQuery {
    #[serde(rename = "deleteImg")]
    delete_img: Option<String>,
}

pub async fn upload_post(
    req: HttpRequest,
    pool: web::Data<Pool>,
    mut payload: Multipart,
) -> impl Responder {
    let user_claims = if let Some(claims) = req.extensions().get::<Claims>() {
        claims.clone()
    } else {
        return HttpResponse::Unauthorized().json(serde_json::json!({
            "status": false,
            "message": "Unauthorized: No valid token found"
        }));
    };

    let mut name_field = String::new();
    let mut description_field = String::new();
    let mut files: Vec<(Vec<u8>, String)> = Vec::new();

    while let Ok(Some(mut field)) = payload.try_next().await {
        let field_name = field.name().to_string();

        if field_name == "postImgs" {
            let cd = field.content_disposition();
            let filename = cd.get_filename().unwrap_or("unknown.jpg").to_string();

            if let Err(e) = Validator::validate_image_type(&filename) {
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "status": false,
                    "message": e
                }));
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

            files.push((bytes, filename));
        } else {
            let mut data = Vec::new();
            while let Some(chunk) = field.try_next().await.unwrap() {
                data.extend_from_slice(&chunk);
            }
            let val = String::from_utf8_lossy(&data).to_string();
            match field_name.as_str() {
                "name" => name_field = val,
                "description" => description_field = val,
                _ => {}
            }
        }
    }

    if let Err(e) = Validator::validate_post_name(&name_field) {
        return HttpResponse::BadRequest().json(serde_json::json!({"status": false, "message": e}));
    }
    if let Err(e) = Validator::validate_post_description(&description_field) {
        return HttpResponse::BadRequest().json(serde_json::json!({"status": false, "message": e}));
    }

    let filenames_only: Vec<String> = files.iter().map(|(_, name)| name.clone()).collect();
    if let Err(e) = Validator::validate_post_images(&filenames_only) {
        return HttpResponse::BadRequest().json(serde_json::json!({"status": false, "message": e}));
    }

    let saved_filenames = match save_multiple_images(files) {
        Ok(v) => v,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .json(serde_json::json!({"status": false, "message": e}));
        }
    };

    let mut conn = pool.get().expect("DB connection error");
    let imgs_db: Vec<Option<String>> = saved_filenames.into_iter().map(Some).collect();

    let inserted = diesel::insert_into(posts::table)
        .values((
            posts::userid.eq(user_claims.id),
            posts::name.eq(name_field),
            posts::description.eq(description_field),
            posts::imgs.eq(imgs_db),
        ))
        .returning((
            posts::id,
            posts::userid,
            posts::name,
            posts::description,
            posts::imgs,
        ))
        .get_result::<(i32, i64, String, String, Vec<Option<String>>)>(&mut conn);

    match inserted {
        Ok((id, userid, name, description, imgs)) => HttpResponse::Created().json(PostResponse {
            status: true,
            message: "User post uploaded successfully".to_string(),
            post: Some(serde_json::json!({
                "id": id,
                "user_id": userid,
                "name": name,
                "description": description,
                "imgs": imgs.into_iter().filter_map(|x| x).collect::<Vec<String>>(),
            })),
        }),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "status": false,
            "message": "Failed to insert post",
            "error": e.to_string(),
        })),
    }
}


pub async fn get_all_posts(
    pool: web::Data<Pool>,
    query: web::Query<std::collections::HashMap<String, String>>,
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

    let joined_query = posts::table
        .inner_join(users::table.on(posts::userid.eq(users::id)))
        .order(posts::created_at.desc())
        .limit(limit)
        .offset(offset)
        .select((
            posts::id,
            posts::userid,
            users::firstname,
            users::lastname,
            users::email,
            users::profile,
            posts::name,
            posts::imgs,
            posts::description,
            posts::created_at,
        ));

    let results = joined_query
        .load::<(
            i32,
            i64,
            String,
            String,
            String,
            String,
            String,
            Vec<Option<String>>,
            String,
            Option<chrono::NaiveDateTime>,
        )>(&mut conn)
        .unwrap_or_default();

    let posts_json = results
        .into_iter()
        .map(
            |(pid, userid, fname, lname, email, profile, name, imgs, desc, created_at)| {
                serde_json::json!({
                    "id": pid,
                    "user_id": userid,
                    "firstname": fname,
                    "lastname": lname,
                    "email": email,
                    "profile": profile,
                    "name": name,
                    "imgs": imgs.into_iter().filter_map(|x| x).collect::<Vec<_>>(),
                    "description": desc,
                    "created_at": created_at,
                })
            },
        )
        .collect();

    let total_count = posts::table
        .count()
        .get_result::<i64>(&mut conn)
        .unwrap_or(0);

    HttpResponse::Ok().json(PostsListResponse {
        status: true,
        posts: posts_json,
        total_post: total_count,
    })
}

// Get one post by id
pub async fn get_post_by_id(pool: web::Data<Pool>, path: web::Path<i32>) -> impl Responder {
    let post_id = path.into_inner();
    let mut conn = pool.get().expect("DB connection error");

    let post_opt = posts::table
        .filter(posts::id.eq(post_id))
        .first::<(
            i32,
            i64,
            String,
            String,
            Vec<Option<String>>,
            Option<chrono::NaiveDateTime>,
        )>(&mut conn)
        .optional();

    match post_opt {
        Ok(Some((pid, userid, name, description, imgs, created_at))) => {
            HttpResponse::Ok().json(serde_json::json!({
                "status": true,
                "post": {
                    "id": pid,
                    "user_id": userid,
                    "name": name,
                    "description": description,
                    "imgs": imgs.into_iter().filter_map(|i| i).collect::<Vec<String>>(),
                    "created_at": created_at,
                }
            }))
        }
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({
            "status": false,
            "message": "Post not found."
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "status": false,
            "message": "Database error",
            "error": e.to_string(),
        })),
    }
}

// Delete post and delete associated images from disk
pub async fn delete_post(pool: web::Data<Pool>, path: web::Path<i32>) -> impl Responder {
    let post_id = path.into_inner();
    let mut conn = pool.get().expect("DB connection error");

    let imgs_opt = posts::table
        .filter(posts::id.eq(post_id))
        .select(posts::imgs)
        .first::<Vec<Option<String>>>(&mut conn)
        .optional();

    match imgs_opt {
        Ok(Some(imgs)) => {
            for img_opt in imgs {
                if let Some(img) = img_opt {
                    let file_path = Path::new("files/userPost").join(&img);
                    if file_path.exists() {
                        if let Err(e) = fs::remove_file(&file_path) {
                            eprintln!("Failed to delete post image {}: {}", img, e);
                        }
                    }
                }
            }
        }
        Ok(None) => {
            return HttpResponse::NotFound().json(serde_json::json!({
                "status": false,
                "message": "Post not found"
            }));
        }
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "status": false,
                "message": "Database error",
                "error": e.to_string(),
            }));
        }
    }

    let deleted = diesel::delete(posts::table.filter(posts::id.eq(post_id))).execute(&mut conn);

    match deleted {
        Ok(count) if count > 0 => HttpResponse::Ok().json(serde_json::json!({
            "status": true,
            "message": "Post is deleted"
        })),
        Ok(_) => HttpResponse::BadRequest().json(serde_json::json!({
            "status": false,
            "message": "Cannot delete the post."
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "status": false,
            "message": "Database error",
            "error": e.to_string(),
        })),
    }
}

// Updated update_post function for post_handler.rs

pub async fn update_post(
    req: HttpRequest,
    pool: web::Data<Pool>,
    path: web::Path<i32>,
    mut payload: Multipart,
) -> impl Responder {
    let post_id = path.into_inner();

    // Check authentication
    let user_claims = if let Some(claims) = req.extensions().get::<Claims>() {
        claims.clone()
    } else {
        return HttpResponse::Unauthorized().json(serde_json::json!({
            "status": false,
            "message": "Unauthorized: No valid token found"
        }));
    };

    let mut conn = pool.get().expect("DB connection error");

    // Get existing post
    let existing_post = posts::table
        .filter(posts::id.eq(post_id))
        .select((
            posts::id,
            posts::userid,
            posts::name,
            posts::description,
            posts::imgs,
        ))
        .first::<(i32, i64, String, String, Vec<Option<String>>)>(&mut conn)
        .optional();

    let (_, post_userid, _, _, mut current_imgs) = match existing_post {
        Ok(Some(post)) => post,
        Ok(None) => {
            return HttpResponse::NotFound().json(serde_json::json!({
                "status": false,
                "message": "Post not found"
            }));
        }
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "status": false,
                "message": "Database error",
                "error": e.to_string()
            }));
        }
    };

    // Check if user owns the post
    if post_userid != user_claims.id {
        return HttpResponse::Forbidden().json(serde_json::json!({
            "status": false,
            "message": "You can only update your own posts"
        }));
    }

    let mut name_field = String::new();
    let mut description_field = String::new();
    let mut new_files: Vec<(Vec<u8>, String)> = Vec::new();
    let mut delete_imgs: Vec<String> = Vec::new();

    // Parse multipart data
    while let Ok(Some(mut field)) = payload.try_next().await {
        let field_name = field.name().to_string();

        if field_name == "postImgs" {
            let cd = field.content_disposition();
            let filename = cd.get_filename().unwrap_or("unknown.jpg").to_string();

            if let Err(e) = Validator::validate_image_type(&filename) {
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "status": false,
                    "message": e
                }));
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

            new_files.push((bytes, filename));
        } else if field_name == "deleteImg" {
            // Collect images to delete from body
            let mut data = Vec::new();
            while let Some(chunk) = field.try_next().await.unwrap() {
                data.extend_from_slice(&chunk);
            }
            let img_name = String::from_utf8_lossy(&data)
                .to_string()
                .trim()
                .to_string();
            if !img_name.is_empty() {
                delete_imgs.push(img_name);
            }
        } else {
            let mut data = Vec::new();
            while let Some(chunk) = field.try_next().await.unwrap() {
                data.extend_from_slice(&chunk);
            }
            let val = String::from_utf8_lossy(&data).to_string();
            match field_name.as_str() {
                "name" => name_field = val,
                "description" => description_field = val,
                _ => {}
            }
        }
    }

    // Handle image deletion
    if !delete_imgs.is_empty() {
        for img_to_delete in &delete_imgs {
            // Delete from disk
            let file_path = Path::new("files/userPost").join(img_to_delete);
            if file_path.exists() {
                if let Err(e) = fs::remove_file(&file_path) {
                    eprintln!("Failed to delete image {}: {}", img_to_delete, e);
                } else {
                    println!("Successfully deleted: {}", img_to_delete);
                }
            } else {
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "status": false,
                    "message": format!("File not found: {}", img_to_delete)
                }));
            }

            // Remove from current_imgs array
            current_imgs.retain(|img_opt| {
                if let Some(img) = img_opt {
                    img != img_to_delete
                } else {
                    true
                }
            });
        }
    }

    // Validate fields if provided
    if !name_field.is_empty() {
        if let Err(e) = Validator::validate_post_name(&name_field) {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "status": false,
                "message": e
            }));
        }
    }

    if !description_field.is_empty() {
        if let Err(e) = Validator::validate_post_description(&description_field) {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "status": false,
                "message": e
            }));
        }
    }

    // Save new images if any
    if !new_files.is_empty() {
        match save_multiple_images(new_files) {
            Ok(saved_filenames) => {
                for filename in saved_filenames {
                    current_imgs.push(Some(filename));
                }
            }
            Err(e) => {
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "status": false,
                    "message": e
                }));
            }
        }
    }

    // Build update query dynamically
    let mut query_builder = diesel::update(posts::table.filter(posts::id.eq(post_id)));

    if !name_field.is_empty() && !description_field.is_empty() {
        let result = query_builder
            .set((
                posts::name.eq(&name_field),
                posts::description.eq(&description_field),
                posts::imgs.eq(&current_imgs),
            ))
            .returning((
                posts::id,
                posts::userid,
                posts::name,
                posts::description,
                posts::imgs,
            ))
            .get_result::<(i32, i64, String, String, Vec<Option<String>>)>(&mut conn);

        match result {
            Ok((id, userid, name, description, imgs)) => {
                return HttpResponse::Ok().json(serde_json::json!({
                    "status": true,
                    "message": "Post updated successfully",
                    "post": {
                        "id": id,
                        "user_id": userid,
                        "name": name,
                        "description": description,
                        "imgs": imgs.into_iter().filter_map(|x| x).collect::<Vec<String>>(),
                    }
                }));
            }
            Err(e) => {
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "status": false,
                    "message": "Failed to update post",
                    "error": e.to_string()
                }));
            }
        }
    } else if !name_field.is_empty() {
        let result = query_builder
            .set((posts::name.eq(&name_field), posts::imgs.eq(&current_imgs)))
            .returning((
                posts::id,
                posts::userid,
                posts::name,
                posts::description,
                posts::imgs,
            ))
            .get_result::<(i32, i64, String, String, Vec<Option<String>>)>(&mut conn);

        match result {
            Ok((id, userid, name, description, imgs)) => {
                return HttpResponse::Ok().json(serde_json::json!({
                    "status": true,
                    "message": "Post updated successfully",
                    "post": {
                        "id": id,
                        "user_id": userid,
                        "name": name,
                        "description": description,
                        "imgs": imgs.into_iter().filter_map(|x| x).collect::<Vec<String>>(),
                    }
                }));
            }
            Err(e) => {
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "status": false,
                    "message": "Failed to update post",
                    "error": e.to_string()
                }));
            }
        }
    } else if !description_field.is_empty() {
        let result = query_builder
            .set((
                posts::description.eq(&description_field),
                posts::imgs.eq(&current_imgs),
            ))
            .returning((
                posts::id,
                posts::userid,
                posts::name,
                posts::description,
                posts::imgs,
            ))
            .get_result::<(i32, i64, String, String, Vec<Option<String>>)>(&mut conn);

        match result {
            Ok((id, userid, name, description, imgs)) => {
                return HttpResponse::Ok().json(serde_json::json!({
                    "status": true,
                    "message": "Post updated successfully",
                    "post": {
                        "id": id,
                        "user_id": userid,
                        "name": name,
                        "description": description,
                        "imgs": imgs.into_iter().filter_map(|x| x).collect::<Vec<String>>(),
                    }
                }));
            }
            Err(e) => {
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "status": false,
                    "message": "Failed to update post",
                    "error": e.to_string()
                }));
            }
        }
    } else {
        // Only images updated
        let result = query_builder
            .set(posts::imgs.eq(&current_imgs))
            .returning((
                posts::id,
                posts::userid,
                posts::name,
                posts::description,
                posts::imgs,
            ))
            .get_result::<(i32, i64, String, String, Vec<Option<String>>)>(&mut conn);

        match result {
            Ok((id, userid, name, description, imgs)) => {
                return HttpResponse::Ok().json(serde_json::json!({
                    "status": true,
                    "message": "Post updated successfully",
                    "post": {
                        "id": id,
                        "user_id": userid,
                        "name": name,
                        "description": description,
                        "imgs": imgs.into_iter().filter_map(|x| x).collect::<Vec<String>>(),
                    }
                }));
            }
            Err(e) => {
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "status": false,
                    "message": "Failed to update post",
                    "error": e.to_string()
                }));
            }
        }
    }
}

use actix_multipart::Multipart;
use actix_web::{HttpMessage, HttpRequest, HttpResponse, Responder, web};
use diesel::prelude::*;
use futures_util::TryStreamExt as _;
use serde::Serialize;
use std::{fs, path::Path};

use crate::{
    db::Pool,
    models::user::{NewPost, Post, PostData, PostWithUser},
    schema::{posts, users},
    utils::auth::Claims,
    utils::{img_upload::save_multiple_images, validation::Validator},
};

#[derive(Serialize)]
struct PostResponse {
    status: bool,
    message: String,
    post: Option<PostData>,
}

#[derive(Serialize)]
struct PostsListResponse {
    status: bool,
    posts: Vec<PostWithUser>,
    total_post: i64,
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

    let imgs_db: Vec<Option<String>> = saved_filenames.into_iter().map(Some).collect();

    let new_post = NewPost {
        userid: user_claims.id,
        name: name_field,
        description: description_field,
        imgs: imgs_db,
    };

    let mut conn = pool.get().expect("DB connection error");

    match diesel::insert_into(posts::table)
        .values(&new_post)
        .get_result::<Post>(&mut conn)
    {
        Ok(post) => {
            let post_data = PostData {
                id: post.id,
                userid: post.userid,
                name: post.name,
                description: post.description,
                imgs: post.imgs,
                created_at: post.created_at,
            };

            HttpResponse::Created().json(PostResponse {
                status: true,
                message: "User post uploaded successfully".to_string(),
                post: Some(post_data),
            })
        }
        Err(e) => {
            eprintln!("Database insert failed: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "status": false,
                "message": "Failed to insert post",
                "error": e.to_string()
            }))
        }
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

    let results = posts::table
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
        ))
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

    let posts_list: Vec<PostWithUser> = results
        .into_iter()
        .map(
            |(pid, userid, fname, lname, email, profile, name, imgs, desc, created_at)| {
                PostWithUser {
                    id: pid,
                    user_id: userid,
                    firstname: fname,
                    lastname: lname,
                    email,
                    profile,
                    name,
                    imgs: imgs.into_iter().filter_map(|x| x).collect(),
                    description: desc,
                    created_at,
                }
            },
        )
        .collect();

    let total_count = posts::table
        .count()
        .get_result::<i64>(&mut conn)
        .unwrap_or(0);

    HttpResponse::Ok().json(PostsListResponse {
        status: true,
        posts: posts_list,
        total_post: total_count,
    })
}

pub async fn get_post_by_id(pool: web::Data<Pool>, path: web::Path<i32>) -> impl Responder {
    let post_id = path.into_inner();
    let mut conn = pool.get().expect("DB connection error");

    let post_result = posts::table
        .filter(posts::id.eq(post_id))
        .first::<Post>(&mut conn)
        .optional();

    match post_result {
        Ok(Some(post)) => {
            let post_data = PostData {
                id: post.id,
                userid: post.userid,
                name: post.name,
                description: post.description,
                imgs: post.imgs,
                created_at: post.created_at,
            };

            HttpResponse::Ok().json(serde_json::json!({
                "status": true,
                "post": post_data
            }))
        }
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({
            "status": false,
            "message": "Post not found"
        })),
        Err(e) => {
            eprintln!("Database query failed: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "status": false,
                "message": "Database error",
                "error": e.to_string()
            }))
        }
    }
}

pub async fn delete_post(pool: web::Data<Pool>, path: web::Path<i32>) -> impl Responder {
    let post_id = path.into_inner();
    let mut conn = pool.get().expect("DB connection error");

    let post_result = posts::table
        .filter(posts::id.eq(post_id))
        .first::<Post>(&mut conn)
        .optional();

    let post = match post_result {
        Ok(Some(p)) => p,
        Ok(None) => {
            return HttpResponse::NotFound().json(serde_json::json!({
                "status": false,
                "message": "Post not found"
            }));
        }
        Err(e) => {
            eprintln!("Database query failed: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "status": false,
                "message": "Database error",
                "error": e.to_string()
            }));
        }
    };

    // Delete images from disk
    for img_opt in post.imgs {
        if let Some(img) = img_opt {
            let file_path = Path::new("files/userPost").join(&img);
            if file_path.exists() {
                if let Err(e) = fs::remove_file(&file_path) {
                    eprintln!("Failed to delete post image {}: {}", img, e);
                }
            }
        }
    }

    match diesel::delete(posts::table.filter(posts::id.eq(post_id))).execute(&mut conn) {
        Ok(count) if count > 0 => HttpResponse::Ok().json(serde_json::json!({
            "status": true,
            "message": "Post deleted successfully"
        })),
        Ok(_) => HttpResponse::BadRequest().json(serde_json::json!({
            "status": false,
            "message": "Cannot delete the post"
        })),
        Err(e) => {
            eprintln!("Database delete failed: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "status": false,
                "message": "Database error",
                "error": e.to_string()
            }))
        }
    }
}

pub async fn update_post(
    req: HttpRequest,
    pool: web::Data<Pool>,
    path: web::Path<i32>,
    mut payload: Multipart,
) -> impl Responder {
    let post_id = path.into_inner();

    let user_claims = if let Some(claims) = req.extensions().get::<Claims>() {
        claims.clone()
    } else {
        return HttpResponse::Unauthorized().json(serde_json::json!({
            "status": false,
            "message": "Unauthorized: No valid token found"
        }));
    };

    let mut conn = pool.get().expect("DB connection error");

    let existing_post = posts::table
        .filter(posts::id.eq(post_id))
        .first::<Post>(&mut conn)
        .optional();

    let mut post = match existing_post {
        Ok(Some(p)) => p,
        Ok(None) => {
            return HttpResponse::NotFound().json(serde_json::json!({
                "status": false,
                "message": "Post not found"
            }));
        }
        Err(e) => {
            eprintln!("Database query failed: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "status": false,
                "message": "Database error",
                "error": e.to_string()
            }));
        }
    };

    if post.userid != user_claims.id {
        return HttpResponse::Forbidden().json(serde_json::json!({
            "status": false,
            "message": "You can only update your own posts"
        }));
    }

    let mut name_field = String::new();
    let mut description_field = String::new();
    let mut new_files: Vec<(Vec<u8>, String)> = Vec::new();
    let mut delete_imgs: Vec<String> = Vec::new();

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
            let val = String::from_utf8_lossy(&data)
                .to_string()
                .trim()
                .to_string();
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
            let file_path = Path::new("files/userPost").join(img_to_delete);
            if file_path.exists() {
                if let Err(e) = fs::remove_file(&file_path) {
                    eprintln!("Failed to delete image {}: {}", img_to_delete, e);
                }
            } else {
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "status": false,
                    "message": format!("File not found: {}", img_to_delete)
                }));
            }

            post.imgs.retain(|img_opt| {
                if let Some(img) = img_opt {
                    img != img_to_delete
                } else {
                    true
                }
            });
        }
    }

    // Update text fields
    if !name_field.is_empty() {
        if let Err(e) = Validator::validate_post_name(&name_field) {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "status": false,
                "message": e
            }));
        }
        post.name = name_field;
    }

    if !description_field.is_empty() {
        if let Err(e) = Validator::validate_post_description(&description_field) {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "status": false,
                "message": e
            }));
        }
        post.description = description_field;
    }

    // Save new images
    if !new_files.is_empty() {
        match save_multiple_images(new_files) {
            Ok(saved_filenames) => {
                for filename in saved_filenames {
                    post.imgs.push(Some(filename));
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

    // Perform update
    let updated_result = diesel::update(posts::table.filter(posts::id.eq(post_id)))
        .set((
            posts::name.eq(&post.name),
            posts::description.eq(&post.description),
            posts::imgs.eq(&post.imgs),
        ))
        .get_result::<Post>(&mut conn);

    match updated_result {
        Ok(updated_post) => {
            let post_data = PostData {
                id: updated_post.id,
                userid: updated_post.userid,
                name: updated_post.name,
                description: updated_post.description,
                imgs: updated_post.imgs,
                created_at: updated_post.created_at,
            };

            HttpResponse::Ok().json(serde_json::json!({
                "status": true,
                "message": "Post updated successfully",
                "post": post_data
            }))
        }
        Err(e) => {
            eprintln!("Database update failed: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "status": false,
                "message": "Failed to update post",
                "error": e.to_string()
            }))
        }
    }
}
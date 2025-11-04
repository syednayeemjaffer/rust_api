use crate::handlers::post_handler::{
    delete_post, get_all_posts, get_post_by_id, update_post, upload_post,
};
use crate::handlers::user_handler::{
    change_password, get_all_users, get_user_by_id, login_user, register_user, update_user,
};
use crate::utils::auth::AuthMiddlewareFactory;
use actix_web::web;

pub fn user_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/register", web::post().to(register_user))
        .route("/login", web::post().to(login_user))
        .service(
            web::scope("")
                .wrap(AuthMiddlewareFactory)
                .route("/users", web::get().to(get_all_users))
                .route("/user/{id}", web::get().to(get_user_by_id))
                .route("/user/{id}", web::put().to(update_user))
                .route("/post", web::post().to(upload_post))
                .route("/allPost", web::get().to(get_all_posts))
                .route("/post/{id}", web::get().to(get_post_by_id))
                .route("/deletePost/{id}", web::delete().to(delete_post))
                .route("/updatePost/{id}", web::put().to(update_post))
                .route("/changePassword/{id}", web::put().to(change_password)),
        );
}

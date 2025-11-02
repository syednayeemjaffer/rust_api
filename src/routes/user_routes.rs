use actix_web::web;
use crate::handlers::user_handler::register_user;

pub fn user_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(register_user);
}

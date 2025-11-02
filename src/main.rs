mod db;
mod schema;
mod models;
mod handlers;
mod routes;
mod utils;

use actix_web::{App, HttpServer, web};
use db::init_pool;
use routes::user_routes::user_routes;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let pool = init_pool();

    println!("🚀 Server running at http://127.0.0.1:8000");

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(pool.clone()))
            .configure(user_routes)
    })
    .bind(("127.0.0.1", 8000))?
    .run()
    .await
}

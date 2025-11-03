mod db;
mod handlers;
mod models;
mod routes;
mod schema;
mod utils;

use actix_files as fs;
use actix_web::{App, HttpServer, web};
use db::init_pool;
use routes::routes::user_routes;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize the database pool
    let pool = init_pool();

    println!("Server running at http://127.0.0.1:8000");

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(pool.clone()))
            .service(
                fs::Files::new("/profile", "./files/usersProfiles")
                    .show_files_listing()
            )
            // Wrap all user routes under /api
            .service(web::scope("/api").configure(user_routes))
    })
    .bind(("127.0.0.1", 8000))?
    .run()
    .await
}

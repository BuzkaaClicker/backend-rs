mod bc;
mod online_users;
mod yt;

use crate::online_users::OnlineUsers;
use actix_web::rt::spawn;
use actix_web::web::Data;
use actix_web::{get, middleware, web, App, HttpServer, Responder};
use anyhow::Context;
use env_logger::Env;
use log::info;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};
use std::env;
use std::sync::{Arc, Mutex};
use crate::bc::CachedChart;

pub mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    env::set_var("RUST_BACKTRACE", "1");
    env_logger::Builder::from_env(Env::new().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    info!("Establishing postgres connection.");
    let pg = create_postgres_pool()
        .await
        .expect("Could not create postgres connection!");
    info!("Established postgres connection.");
    let online_users = Data::new(Mutex::new(OnlineUsers::new()));

    spawn(online_users::start_archiving(
        Pool::clone(&pg),
        Data::clone(&online_users),
    ));

    let cached_chart = CachedChart::new(Pool::clone(&pg))
        .await
        .context("Could not create cached chart!")?;
    let chart_data = Data::new(futures::lock::Mutex::new(cached_chart));
    HttpServer::new(move || {
        App::new()
            .app_data(Data::clone(&online_users))
            .app_data(Data::new(Pool::clone(&pg)))
            .service(index)
            .service(
                web::scope("/buzkaaclicker")
                    .app_data(Data::clone(&chart_data))
                    .service(bc::get_chart)
                    .service(
                        web::resource(vec!["/online-users", "/onlineUsers"])
                            .route(web::get().to(bc::get_online_users_count)),
                    )
                    .service(bc::version),
            )
            .service(web::scope("/youtube").service(yt::live))
            .wrap(middleware::Logger::default())
    })
    .bind(("127.0.0.1", 2137))?
    .run()
    .await?;
    Ok(())
}

async fn create_postgres_pool() -> anyhow::Result<Pool<Postgres>> {
    let url = env::var("POSTGRES_URL").context("Could not get POSTGRES_URL env")?;
    PgPoolOptions::new()
        .max_connections(10)
        .connect(&url)
        .await
        .context("Could not connect to postgres")
}

#[get("/")]
async fn index() -> impl Responder {
    format!(
        "🦀 {} v{} - {:?} by {} at {} 🦀",
        built_info::PKG_NAME,
        built_info::PKG_VERSION,
        built_info::GIT_VERSION.unwrap_or("*git commit id missing*"),
        built_info::PKG_AUTHORS,
        built_info::BUILT_TIME_UTC,
    )
}

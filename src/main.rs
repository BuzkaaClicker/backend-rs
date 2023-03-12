#![feature(result_option_inspect)]
#![feature(result_flattening)]

use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use actix_extensible_rate_limit::backend::memory::InMemoryBackend;
use actix_extensible_rate_limit::backend::{SimpleInputFunctionBuilder, SimpleOutput};
use actix_extensible_rate_limit::RateLimiter;
use actix_web::http::header::ContentType;
use actix_web::rt::spawn;
use actix_web::web::Data;
use actix_web::{get, middleware, web, App, HttpResponse, HttpServer, Responder};
use anyhow::Context;
use env_logger::Env;
use log::info;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};

use crate::bc::CachedChart;
use crate::file_host::FileHost;
use crate::online_users::OnlineUsers;
use crate::yt::CachedLiveVisitor;

mod bc;
mod file_host;
mod online_users;
mod yt;

pub mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    env::set_var("RUST_BACKTRACE", "1");
    env_logger::Builder::from_env(Env::new().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    let bc_version: u32 = env::var("BUZKAACLICKER_VERSION")
        .context("Invalid BUZKAACLICKER_VERSION env variable!")?
        .parse()
        .context("BUZKAACLICKER_VERSION is not a u32 number!")?;
    let bc_version = bc::Version(bc_version);

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
    let chart_data = create_chart_data(&pg).await?;
    let file_host = create_file_host(Pool::clone(&pg));
    let rate_limiter_backend = InMemoryBackend::builder().build();
    let live_visitor = Data::new(futures::lock::Mutex::new(
        CachedLiveVisitor::new()
            .await
            .context("Could not create cached visitor")?,
    ));

    HttpServer::new(move || {
        let input = SimpleInputFunctionBuilder::new(Duration::from_secs(5), 1)
            .real_ip_key()
            .build();
        let rate_limiter = RateLimiter::builder(rate_limiter_backend.clone(), input)
            .add_headers()
            .request_denied_response(rate_limited)
            .build();
        App::new()
            .app_data(Data::clone(&online_users))
            .app_data(Data::new(Pool::clone(&pg)))
            .app_data(Data::clone(&file_host))
            .app_data(Data::clone(&live_visitor))
            .service(index)
            .service(
                web::scope("/buzkaaclicker")
                    .app_data(Data::clone(&chart_data))
                    .app_data(Data::new(bc_version))
                    .service(bc::get_chart)
                    .service(
                        web::resource(vec!["/online-users", "/onlineUsers"])
                            .route(web::get().to(bc::get_online_users_count)),
                    )
                    .service(bc::version),
            )
            .service(web::scope("/youtube").service(yt::live))
            .service(
                web::scope("")
                    .service(file_host::download_specific)
                    .wrap(rate_limiter),
            )
            .wrap(middleware::Logger::new(
                r#"%a (%{r}a) "%r" %s %b "%{Referer}i" "%{User-Agent}i" %T"#,
            ))
    })
    .bind(("127.0.0.1", 2137))?
    .run()
    .await?;
    Ok(())
}

async fn create_chart_data(
    pg: &Pool<Postgres>,
) -> anyhow::Result<Data<futures::lock::Mutex<CachedChart>>> {
    let cached_chart = CachedChart::new(Pool::clone(&pg))
        .await
        .context("Could not create cached chart!")?;
    Ok(Data::new(futures::lock::Mutex::new(cached_chart)))
}

fn create_file_host(pg: Pool<Postgres>) -> Data<FileHost> {
    let files = HashMap::from([
        (
            "BuzkaaClickerInstaller".into(),
            PathBuf::from("./filehost/BuzkaaClickerInstaller.webp.zip"),
        ),
        (
            "BuzkaaClickerInstaller2".into(),
            PathBuf::from("./filehost/BuzkaaClickerInstaller.txt"),
        ),
    ]);
    Data::new(FileHost::new(
        Pool::clone(&pg),
        String::from("BuzkaaClickerInstaller"),
        files,
    ))
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

fn rate_limited(_: &SimpleOutput) -> HttpResponse {
    HttpResponse::TooManyRequests()
        .content_type(ContentType::html())
        .body(
            r#"
            <html>
            <body 
                style="background: #111; color: #fafafa; font-family: sans-serif; display: flex; 
                    justify-content: center; align-items: center;"
            >
                <h1>Zwolnij...</h1>
            </body>
            </html>
            "#,
        )
}

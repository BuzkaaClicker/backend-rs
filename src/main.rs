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
use actix_web::middleware::DefaultHeaders;
use actix_web::rt::spawn;
use actix_web::web::Data;
use actix_web::{get, middleware, web, App, HttpResponse, HttpServer, Responder};
use anyhow::Context;
use env_logger::Env;
use log::info;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};

use crate::bc::{ChartJson, DownloadCount};
use crate::file_host::FileHost;
use crate::online_users::OnlineUsers;
use crate::yt::LiveJson;

mod bc;
mod cache;
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
    let chart_json = Data::new(ChartJson::memoized(Pool::clone(&pg)).await);
    let file_host = create_file_host(Pool::clone(&pg));
    let rate_limiter_backend = InMemoryBackend::builder().build();
    let live_json = Data::new(LiveJson::memoized().await);
    let download_counter = Data::new(DownloadCount::memoized(Pool::clone(&pg)).await);

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
            .app_data(Data::clone(&live_json))
            .app_data(Data::clone(&download_counter))
            .wrap(DefaultHeaders::new().add(("Access-Control-Allow-Origin", "*")))
            .service(index)
            .configure(|app_config| {
                for path in ["/buzkaaClicker", "/buzkaaclicker"] {
                    app_config.service(web::scope(path).configure(|config| {
                        bc::configure_service(bc_version, &chart_json, config)
                    }));
                }
            })
            .service(web::scope("/youtube").service(yt::live))
            .service(
                web::resource(["/download", "/download/", "/download/{file}"])
                    .route(web::get().to(file_host::download_specific))
                    .wrap(rate_limiter),
            )
            .wrap(
                middleware::Logger::new(
                    r#"%a (%{r}a) "%r" %s %b "%{Referer}i" "%{User-Agent}i" %T"#,
                )
                .exclude("/youtube/Buzkaa"),
            )
    })
    .bind(("0.0.0.0", 2137))?
    .run()
    .await?;
    Ok(())
}

fn create_file_host(pg: Pool<Postgres>) -> Data<FileHost> {
    let files = HashMap::from([
        (
            "BClickerDownloader".into(),
            PathBuf::from("./filehost/BClickerDownloader.zip"),
        ),
        (
            "BuzkaaClicker".into(),
            PathBuf::from("./filehost/BuzkaaClicker-v16.rar"),
        ),
    ]);
    Data::new(FileHost::new(
        Pool::clone(&pg),
        String::from("BClickerDownloader"),
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

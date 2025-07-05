use crate::cache::Memoized;
use crate::online_users;
use crate::online_users::OnlineUsersData;
use actix_web::http::header::ContentType;
use actix_web::web::{Data, ServiceConfig};
use actix_web::{get, web, HttpResponse, Responder};
use anyhow::Context;
use log::{debug, error};
use sqlx::{Pool, Row, Sqlite};
use std::time::Duration;

pub fn configure_service(
    bc_version: Version,
    chart_json: &Data<Memoized<ChartJson>>,
    config: &mut ServiceConfig,
) {
    config
        .app_data(Data::clone(chart_json))
        .app_data(Data::new(bc_version))
        .service(get_chart)
        .service(get_download_count)
        .service(
            web::resource(vec!["/online-users", "/onlineUsers"])
                .route(web::get().to(get_online_users_count)),
        )
        .service(version);
}

#[derive(Copy, Clone)]
pub struct Version(pub u32);

pub async fn get_online_users_count(online_users: OnlineUsersData) -> impl Responder {
    let count = online_users.lock().expect("Online users poisoned").count();
    format!("{count}")
}

#[derive(Clone)]
pub struct ChartJson(Option<String>);

impl ChartJson {
    pub async fn memoized(pg: Pool<Sqlite>) -> Memoized<Self> {
        Memoized::new(Duration::from_secs(60), move || {
            let pg = Pool::clone(&pg);
            async move {
                debug!("Generating new chart json...");
                let result = online_users::get_chart_data(&pg)
                    .await
                    .context("Could not get chart data from db")
                    .and_then(|data| {
                        serde_json::to_string(&data).context("Could not serialize chart data")
                    });
                Self(match result {
                    Ok(json) => Some(json),
                    Err(err) => {
                        error!("Could not get chart data from db: {err:#}");
                        None
                    }
                })
            }
        })
        .await
    }
}

#[get("/online-list")]
pub async fn get_chart(chart: web::Data<Memoized<ChartJson>>) -> actix_web::Result<impl Responder> {
    let chart_data = chart
        .get()
        .await
        .0
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("no data"))?
        .to_string();
    Ok(HttpResponse::Ok()
        .content_type(ContentType::json())
        .body(chart_data))
}

#[derive(Clone)]
pub struct DownloadCount(Option<u64>);

impl DownloadCount {
    pub async fn memoized(pg: Pool<Sqlite>) -> Memoized<Self> {
        Memoized::new(Duration::from_secs(60), move || {
            let pg = Pool::clone(&pg);
            async move {
                let row_result = sqlx::query(
                    "SELECT COUNT(DISTINCT ip) FROM downloads WHERE file='BClickerDownloader'",
                )
                .fetch_one(&pg)
                .await
                .context("Could not select data");
                Self(match row_result {
                    Ok(row) => Some(row.get::<i64, _>(0) as u64),
                    Err(err) => {
                        error!("Could not get download count from db: {err:#}");
                        None
                    }
                })
            }
        })
        .await
    }
}

#[get("/download-count")]
pub async fn get_download_count(
    download_counter: web::Data<Memoized<DownloadCount>>,
) -> actix_web::Result<impl Responder> {
    let resp = match download_counter.get().await.0 {
        None => HttpResponse::InternalServerError().finish(),
        Some(count) => HttpResponse::Ok().body(count.to_string()),
    };
    Ok(resp)
}

#[get("/version")]
pub async fn version(version: web::Data<Version>) -> impl Responder {
    version.0.to_string()
}

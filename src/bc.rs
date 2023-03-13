use crate::online_users;
use crate::online_users::OnlineUsersData;
use actix_web::http::header::ContentType;
use actix_web::{get, web, HttpResponse, Responder};
use anyhow::{Context, Error};
use futures::lock::Mutex;
use log::{debug, error};
use sqlx::postgres::PgRow;
use sqlx::{Pool, Postgres, Row};
use std::time::{Duration, Instant};

#[derive(Copy, Clone)]
pub struct Version(pub u32);

pub async fn get_online_users_count(online_users: OnlineUsersData) -> impl Responder {
    let count = online_users.lock().expect("Online users poisoned").count();
    format!("{count}")
}

pub struct CachedChart {
    pg: Pool<Postgres>,
    json: String,
    last_update: Instant,
}

impl CachedChart {
    const CACHE_DURATION_SECS: u64 = 60;

    pub async fn new(pg: Pool<Postgres>) -> anyhow::Result<Self> {
        let json = CachedChart::fetch_json(&pg).await?;
        Ok(Self {
            pg,
            json,
            last_update: Instant::now(),
        })
    }

    pub async fn get_json(&mut self) -> anyhow::Result<&str> {
        if self.last_update.elapsed().as_secs() < CachedChart::CACHE_DURATION_SECS {
            return Ok(&self.json);
        }
        debug!("Generating new chart json...");
        self.json = CachedChart::fetch_json(&self.pg).await?;
        self.last_update = Instant::now();
        Ok(&self.json)
    }

    async fn fetch_json(pg: &Pool<Postgres>) -> anyhow::Result<String> {
        let chart_data = online_users::get_chart_data(pg)
            .await
            .context("Could not get chart data from db")?;
        serde_json::to_string(&chart_data).context("Could not serialize chart data")
    }
}

#[get("/online-list")]
pub async fn get_chart(chart: web::Data<Mutex<CachedChart>>) -> actix_web::Result<impl Responder> {
    let chart_data = chart
        .lock()
        .await
        .get_json()
        .await
        .context("Could not get json chart")
        .map_err(actix_web::error::ErrorInternalServerError)?
        .to_string();
    Ok(HttpResponse::Ok()
        .content_type(ContentType::json())
        .body(chart_data))
}

pub struct DownloadCounter {
    pg: Pool<Postgres>,
    count: u64,
    refreshed: Instant,
}

impl DownloadCounter {
    pub async fn new(pg: Pool<Postgres>) -> Self {
        let count = DownloadCounter::fetch_count(&pg)
            .await
            .expect("Cannot fetch download counter initial value!");
        Self {
            pg,
            count,
            refreshed: Instant::now(),
        }
    }

    pub async fn get_count(&mut self) -> u64 {
        if self.refreshed.elapsed() < Duration::from_secs(30) {
            return self.count;
        }
        self.refreshed = Instant::now();
        if let Some(count) = DownloadCounter::fetch_count(&self.pg).await {
            self.count = count
        }
        self.count
    }

    async fn fetch_count(pg: &Pool<Postgres>) -> Option<u64> {
        let row_result = sqlx::query(
            "SELECT COUNT(DISTINCT ip) FROM downloads WHERE file='BuzkaaClickerInstaller'",
        )
        .fetch_one(pg)
        .await
        .context("Could not select data");
        match row_result {
            Ok(row) => Some(row.get::<i64, _>(0) as u64),
            Err(err) => {
                error!("Could not get download count from db: {err:#}");
                None
            }
        }
    }
}

#[get("/download-count")]
pub async fn get_download_count(
    download_counter: web::Data<Mutex<DownloadCounter>>,
) -> actix_web::Result<impl Responder> {
    let count = download_counter.lock().await.get_count().await.to_string();
    Ok(HttpResponse::Ok().body(count))
}

#[get("/version")]
pub async fn version(version: web::Data<Version>) -> impl Responder {
    version.0.to_string()
}

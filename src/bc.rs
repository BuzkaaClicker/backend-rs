use crate::online_users;
use crate::online_users::OnlineUsersData;
use actix_web::http::header::ContentType;
use actix_web::{get, web, HttpResponse, Responder};
use anyhow::Context;
use futures::lock::Mutex;
use log::debug;
use sqlx::{Pool, Postgres};
use std::time::Instant;

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

#[get("/version")]
pub async fn version(version: web::Data<Version>) -> impl Responder {
    version.0.to_string()
}

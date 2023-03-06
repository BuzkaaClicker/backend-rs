use actix_web::rt::time;
use actix_web::web;
use anyhow::Context;
use built::chrono::NaiveDateTime;
use log::{error, info};
use sqlx::{Pool, Postgres, Row};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub type OnlineUsersData = web::Data<Mutex<OnlineUsers>>;

pub struct OnlineUsers {
    users: HashMap<String, Instant>,
    last_cleanup_at: Instant,
}

impl OnlineUsers {
    pub fn new() -> Self {
        Self {
            users: Default::default(),
            last_cleanup_at: Instant::now(),
        }
    }

    fn cleanup(&mut self) {
        self.last_cleanup_at = Instant::now();
        self.users
            .retain(|_, visited_at| visited_at.elapsed().as_secs() < 70);
    }

    pub fn count(&mut self) -> u32 {
        if self.last_cleanup_at.elapsed().as_secs() >= 10 {
            self.cleanup()
        }
        self.users.len() as u32
    }

    pub fn keep_alive(&mut self, ip: String) {
        self.users.insert(ip, Instant::now());
    }
}

async fn insert_online_users(pg: &Pool<Postgres>, online_count: u32) -> anyhow::Result<()> {
    sqlx::query("INSERT INTO online_users (time, count) VALUES (NOW(), $1)")
        .bind(online_count as i32)
        .execute(pg)
        .await
        .context("Could not insert record")?;
    Ok(())
}

pub async fn start_archiving(pg: Pool<Postgres>, online_users_data: OnlineUsersData) {
    let mut store_interval = time::interval(Duration::from_secs(60));
    store_interval.tick().await;
    loop {
        store_interval.tick().await;
        let online_count = online_users_data
            .lock()
            .expect("Online users poisoned!")
            .count();
        match insert_online_users(&pg, online_count).await {
            Ok(_) => {
                info!("Archived online users (count: {online_count}).");
            }
            Err(err) => {
                error!("Could not archive online users (count: {online_count}): {err:#}.");
            }
        }
    }
}

#[derive(serde::Serialize)]
pub struct ChartData {
    #[serde(rename = "labels")]
    time_labels: Vec<String>,
    #[serde(rename = "data")]
    counts: Vec<u32>,
}

pub async fn get_chart_data(pg: &Pool<Postgres>) -> anyhow::Result<ChartData> {
    let rows =
        sqlx::query("SELECT time, count FROM online_users ORDER BY id DESC LIMIT 60 * 24 * 7;")
            .fetch_all(pg)
            .await
            .context("Could not select data")?;
    let mut time_labels = Vec::with_capacity(rows.len());
    let mut counts = Vec::with_capacity(rows.len());
    for row in rows {
        let time = row
            .try_get::<NaiveDateTime, _>("time")
            .expect("time must be NaiveDateTime");
        let count = row.try_get::<i32, _>("count").expect("count must be i32") as u32;
        let time_formatted = time.format("%d.%m %H:%M").to_string();
        time_labels.push(time_formatted);
        counts.push(count);
    }
    Ok(ChartData {
        time_labels,
        counts,
    })
}

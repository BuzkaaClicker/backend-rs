use std::sync::Arc;
use std::time::{Duration};

use crate::cache::Memoized;
use actix_web::http::header::ContentType;
use actix_web::{get, web, HttpRequest, HttpResponse, Responder};
use anyhow::{anyhow, Context};
use chrono::{DateTime, FixedOffset};
use log::{debug, error};
use scraper::{Html, Selector};
use serde::Serialize;

use crate::online_users::OnlineUsersData;

const CHANNEL_ID: &str = "UCL1s7OtDPaX3SdhW5433PRw";
const LIVE_URL: &str = "https://www.youtube.com/channel/UCL1s7OtDPaX3SdhW5433PRw/live";

pub struct LiveVisitor {
    name_sel: Selector,
    start_date_sel: Selector,
}

impl LiveVisitor {
    pub fn new() -> Self {
        let name_sel = Selector::parse(r#"#watch7-content > meta[itemprop="name"]"#)
            .expect("Invalid name selector");
        let start_date_sel = Selector::parse(r#"#watch7-content > * > meta[itemprop="startDate"]"#)
            .expect("Invalid name selector");
        Self {
            name_sel,
            start_date_sel,
        }
    }

    // aint gonna pay for api
    pub async fn get_live_meta(&self) -> anyhow::Result<Option<LiveMeta>> {
        debug!("Scrapping live meta...");
        let client = awc::Client::default();
        let mut res = client
            .get(LIVE_URL)
            .insert_header(("User-Agent", "curl"))
            .send()
            .await
            .map_err(|err| anyhow!("Could not visit youtube channel page: {}", err))?;
        let body = res
            .body()
            .await
            .context("Could not read youtube channel page body")?;
        let body = String::from_utf8_lossy(&body);
        let document = Html::parse_document(&body);
        let title = match document.select(&self.name_sel).next() {
            None => return Ok(None),
            Some(element) => element
                .value()
                .attr("content")
                .context("Could not select title content")?
                .to_owned(),
        };
        let start_date_raw = match document.select(&self.start_date_sel).next() {
            None => return Ok(None),
            Some(element) => element
                .value()
                .attr("content")
                .context("Could not select start date content")?,
        };
        let start_date =
            DateTime::parse_from_rfc3339(start_date_raw).context("Could not parse start date!")?;
        debug!("Live stream scrapped. Title: '{title}', start date: '{start_date_raw}'");
        Ok(Some(LiveMeta { title, start_date }))
    }
}

#[derive(Debug, Clone)]
pub struct LiveMeta {
    title: String,
    start_date: DateTime<FixedOffset>,
}

#[derive(Serialize, Default, Clone)]
#[serde(rename_all = "PascalCase")]
struct LiveResponse {
    pub id: String,
    pub name: String,
    pub live_stream_title: String,
    pub live_streaming: bool,
    pub live_stream_url: String,
    pub live_stream_start_time: String,
}

impl From<Option<LiveMeta>> for LiveResponse {
    fn from(meta_maybe: Option<LiveMeta>) -> Self {
        match meta_maybe {
            None => LiveResponse {
                id: String::from(CHANNEL_ID),
                name: String::new(),
                live_stream_title: String::new(),
                live_streaming: false,
                live_stream_url: String::new(),
                live_stream_start_time: String::new(),
            },
            Some(meta) => LiveResponse {
                id: String::from(CHANNEL_ID),
                name: String::from("Buzkaa"),
                live_stream_title: meta.title,
                live_streaming: true,
                live_stream_url: String::from(LIVE_URL),
                live_stream_start_time: meta.start_date.to_rfc3339(),
            },
        }
    }
}

#[derive(Clone)]
pub struct LiveJson(Option<String>);

impl LiveJson {
    pub async fn memoized() -> Memoized<Self> {
        let visitor = Arc::new(LiveVisitor::new());
        Memoized::new(Duration::from_secs(60), move || {
            let visitor = Arc::clone(&visitor);
            async move {
                debug!("Generating new live json...");
                let result = visitor
                    .get_live_meta()
                    .await
                    .map(LiveResponse::from)
                    .and_then(|response| {
                        serde_json::to_string(&response)
                            .context("Could not serialize live meta response")
                    });
                Self(match result {
                    Ok(json) => Some(json),
                    Err(err) => {
                        error!("Could not fetch live json: {err}");
                        None
                    }
                })
            }
        })
        .await
    }
}

#[get("/Buzkaa")]
pub async fn live(
    req: HttpRequest,
    online_users: OnlineUsersData,
    live_json: web::Data<Memoized<LiveJson>>,
) -> actix_web::Result<impl Responder> {
    let ip = req
        .connection_info()
        .realip_remote_addr()
        .expect("Could not get real ip.")
        .to_string();
    online_users
        .lock()
        .expect("Online users poisoned!")
        .keep_alive(ip);
    let live_meta = live_json
        .get()
        .await
        .0
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("Could not get live metadata"))?
        .to_owned();
    Ok(HttpResponse::Ok()
        .content_type(ContentType::json())
        .body(live_meta))
}

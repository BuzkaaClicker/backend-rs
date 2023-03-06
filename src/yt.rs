use crate::online_users::OnlineUsersData;
use actix_web::{get, web, HttpRequest, Responder};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct LiveResponse {
    pub id: String,
    pub name: String,
    pub live_stream_title: String,
    pub live_streaming: bool,
    pub live_stream_url: String,
    pub live_stream_start_time: String,
}

#[get("/Buzkaa")]
pub async fn live(req: HttpRequest, online_users: OnlineUsersData) -> impl Responder {
    let ip = req
        .connection_info()
        .realip_remote_addr()
        .expect("Could not get real ip.")
        .to_string();
    online_users
        .lock()
        .expect("Online users poisoned!")
        .keep_alive(ip);

    web::Json(LiveResponse {
        id: "".to_string(),
        name: "".to_string(),
        live_stream_title: "".to_string(),
        live_streaming: false,
        live_stream_url: "".to_string(),
        live_stream_start_time: "".to_string(),
    })
}

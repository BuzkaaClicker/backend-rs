use actix_files::NamedFile;
use actix_web::{web, HttpRequest};
use anyhow::Context;
use log::error;
use sqlx::{Pool, Sqlite};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct FileHost {
    pg: Pool<Sqlite>,
    default_file: String,
    files: HashMap<String, PathBuf>,
}

impl FileHost {
    pub fn new(pg: Pool<Sqlite>, default_file: String, files: HashMap<String, PathBuf>) -> Self {
        let files_lowercase = HashMap::from_iter(
            files
                .into_iter()
                .map(|(name, path)| (name.to_lowercase(), path)),
        );
        Self {
            pg,
            default_file,
            files: files_lowercase,
        }
    }

    pub async fn download(
        &self,
        ip: &str,
        file_name: &Option<&str>,
    ) -> actix_web::Result<NamedFile> {
        let file_name = file_name.unwrap_or(&self.default_file);
        let file_path = self
            .files
            .get(&file_name.to_lowercase())
            .ok_or(actix_web::error::ErrorNotFound("file not found"))?;
        if let Err(err) = self.insert_stat(ip, file_name).await {
            error!("Could not insert download statistic to db: {:#}", err);
        }
        NamedFile::open_async(file_path)
            .await
            .map(|file| file.use_etag(true).use_last_modified(true))
            .inspect_err(|err| error!("Could not open named file: {err}"))
            .map_err(|_| actix_web::error::ErrorInternalServerError("could not serve file"))
    }

    async fn insert_stat(&self, ip: &str, file_name: &str) -> anyhow::Result<()> {
        sqlx::query("insert into downloads (time, ip, file) values (datetime('now'), ?, ?);")
            .bind(ip)
            .bind(file_name)
            .execute(&self.pg)
            .await
            .context("Could not insert stat record!")?;
        Ok(())
    }
}

pub async fn download_specific(
    req: HttpRequest,
    file_host: web::Data<FileHost>,
) -> actix_web::Result<NamedFile> {
    let file_name = req.match_info().get("file");
    let ip = req
        .connection_info()
        .realip_remote_addr()
        .expect("Request ip must be present!")
        .to_owned();
    file_host.download(&ip, &file_name).await
}

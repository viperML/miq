use std::fs::Permissions;
use std::os::unix::prelude::PermissionsExt;
use std::sync::Mutex;

use async_trait::async_trait;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use tracing::debug;

use crate::db::DbConnection;
use crate::schema_eval::{Build, Fetch};
use crate::*;

#[async_trait]
impl Build for Fetch {
    #[tracing::instrument(skip(conn), ret, err, level = "debug")]
    async fn build(
        &self,
        rebuild: bool,
        conn: &Mutex<DbConnection>,
        pb: Option<ProgressBar>,
    ) -> Result<()> {
        let path = self.result.store_path();
        let path = path.as_path();

        if conn.lock().unwrap().is_db_path(&path)? {
            if rebuild {
                conn.lock().unwrap().remove(&path)?;
            } else {
                return Ok(());
            }
        }

        crate::build::clean_path(path)?;

        let client = reqwest::Client::new();
        let response = client.get(&self.url).send().await?;

        let status = response.status();
        if !status.is_success() {
            bail!(status);
        }

        let pb = pb.wrap_err("Didn't receive a progress bar!")?;

        if let Some(total_length) = response.content_length() {
            pb.set_length(total_length);
            pb.set_style(ProgressStyle::with_template(
                "{msg:.blue}>> {percent}% {wide_bar}",
            )?);
            pb.set_message(self.name.clone());
        };

        let out_file = tokio::fs::File::create(path).await?;

        let mut out_writer = pb.wrap_async_write(out_file);
        let mut stream = response.bytes_stream();
        while let Some(item) = stream.next().await {
            let item = item?;
            tokio::io::copy(&mut item.as_ref(), &mut out_writer).await?;
        }

        let perm = if self.executable {
            debug!("Setting as executable exec bit");
            Permissions::from_mode(0o555)
        } else {
            Permissions::from_mode(0o444)
        };

        tokio::fs::set_permissions(path, perm).await?;

        pb.finish_and_clear();

        conn.lock().unwrap().add(&path)?;
        Ok(())
    }
}

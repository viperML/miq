use std::fs::Permissions;
use std::os::unix::prelude::PermissionsExt;
use std::sync::Mutex;

use async_trait::async_trait;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use tokio::io::AsyncWriteExt;
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
        pb: ProgressBar,
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

        if let Some(total_length) = response.content_length() {
            pb.set_length(total_length);
            pb.set_message(self.name.clone());
            pb.set_style(ProgressStyle::with_template(
                "{msg:.blue}>> {percent}% {wide_bar}",
            )?);
        };

        let out_file = tokio::fs::File::create(path).await?;

        let pb_writer = pb.wrap_async_write(out_file);
        let mut buf_writer = tokio::io::BufWriter::new(pb_writer);
        let mut stream = response.bytes_stream();

        while let Some(item) = stream.next().await {
            let item = item?;
            tokio::io::copy(&mut item.as_ref(), &mut buf_writer).await?;
        }

        buf_writer.flush().await?;

        let perm = if self.executable {
            debug!("Setting as executable exec bit");
            Permissions::from_mode(0o555)
        } else {
            Permissions::from_mode(0o444)
        };

        tokio::fs::set_permissions(path, perm).await?;


        conn.lock().unwrap().add(&path)?;
        pb.finish_and_clear();
        Ok(())
    }
}

use std::fs::Permissions;
use std::os::unix::prelude::PermissionsExt;
use std::sync::Mutex;

use async_trait::async_trait;
use futures_util::{Stream, StreamExt};
use indicatif::ProgressBar;
use tracing::{debug, trace, warn};

use crate::db::DbConnection;
use crate::schema_eval::{Build, Fetch};
use crate::*;

#[async_trait]
impl Build for Fetch {
    #[tracing::instrument(skip(conn), ret, err, level = "debug")]
    async fn build(&self, rebuild: bool, conn: &Mutex<DbConnection>, pb: Option<ProgressBar>) -> Result<()> {
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

        let total_length = response.content_length();

        let mut out = tokio::fs::File::create(path).await?;
        let mut stream = response.bytes_stream();
        while let Some(item) = stream.next().await {
            let hint = stream.size_hint();
            warn!(?hint);
            tokio::io::copy(&mut item?.as_ref(), &mut out).await?;
        }

        let perms = if self.executable {
            debug!("Setting as executable exec bit");
            Permissions::from_mode(0o555)
        } else {
            Permissions::from_mode(0o444)
        };

        out.set_permissions(perms).await?;

        conn.lock().unwrap().add(&path)?;
        Ok(())
    }
}

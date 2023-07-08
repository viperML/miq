use std::ffi::OsStr;
use std::sync::Mutex;

use async_trait::async_trait;
use bytes::Buf;
use daggy::Walker;
use futures::StreamExt;
use tracing::{debug, trace};

use crate::db::DbConnection;
use crate::schema_eval::{Build, Fetch};
use crate::*;

#[async_trait]
impl Build for Fetch {
    #[tracing::instrument(skip(conn), ret, err, level = "debug")]
    async fn build(&self, rebuild: bool, conn: &Mutex<DbConnection>) -> Result<()> {
        let path = self.result.store_path();
        let path = path.as_path();

        if conn.lock().unwrap().is_db_path(&path)? {
            if rebuild {
                conn.lock().unwrap().remove(&path)?;
            } else {
                return Ok(());
            }
        }

        let tempfile = &mut tempfile::NamedTempFile::new()?;
        debug!(?tempfile);

        let client = reqwest::Client::new();
        trace!("Fetching file, please wait");
        let response = client.get(&self.url).send().await?;
        let content = &mut response.bytes().await?.reader();
        std::io::copy(content, tempfile)?;

        std::fs::copy(tempfile.path(), &path)?;

        if self.executable {
            // FIXME
            debug!("Setting exec bit");
            std::process::Command::new("chmod")
                .args([OsStr::new("+x"), path.as_ref()])
                .output()?;
        }

        conn.lock().unwrap().add(&path)?;
        Ok(())
    }
}

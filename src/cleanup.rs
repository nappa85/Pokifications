use std::time;

use chrono::{Duration, Utc};

use chrono_tz::Europe::Rome;

use tokio::{
    fs::{read_dir, remove_file},
    time::interval,
};

use tracing::error;

use crate::config::CONFIG;

async fn cleanup() -> Result<(), ()> {
    let mut dir = read_dir(format!("{}img_sent/", CONFIG.images.bot,))
        .await
        .map_err(|e| error!("cleanup error: can't open dir {}", e))?;

    let search = (Utc::now() - Duration::hours(2))
        .with_timezone(&Rome)
        .format("%Y%m%d%H")
        .to_string();
    while let Ok(Some(file)) = dir
        .next_entry()
        .await
        .map_err(|e| error!("cleanup error: can't read dir {}", e))
    {
        if file.file_name().to_str().and_then(|f| f.split('_').nth(1)) == Some(search.as_str()) {
            if let Err(e) = remove_file(file.path()).await {
                error!("cleanup error: can't remove file {}", e);
            }
        }
    }

    Ok(())
}

pub fn init() {
    tokio::spawn(async {
        let mut interval = interval(time::Duration::from_secs(3600));
        loop {
            interval.tick().await;

            cleanup().await.ok();
        }
    });
}

//! Faz 10b spike — librqbit canlı indirme doğrulaması.
//!
//! Gerçek bir public torrent indirir.
//! Kullanım: `cargo run --release -p manager-torrent --example live_torrent <output_dir>`
//!
//! Bu örnek Faz 10b'nin fizibilite kanıtıdır.

use std::time::Duration;

use librqbit::{AddTorrent, AddTorrentOptions, AddTorrentResponse, Session};

const MAGNET_LINK: &str = "magnet:?xt=urn:btih:cab507494d02ebb1178b38f2e9d7be299c86b862";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
        )
        .init();

    let output_dir = std::env::args()
        .nth(1)
        .expect("ilk argüman çıktı dizini olmalı");

    let session = Session::new(output_dir.into()).await?;
    tracing::info!("librqbit session oluşturuldu (aarch64 Linux)");

    let handle = match session
        .add_torrent(
            AddTorrent::from_url(MAGNET_LINK.to_string()),
            Some(AddTorrentOptions {
                overwrite: true,
                ..Default::default()
            }),
        )
        .await?
    {
        AddTorrentResponse::Added(_, handle) => handle,
        _ => return Err("beklenmeyen AddTorrentResponse".into()),
    };

    handle.with_metadata(|r| {
        tracing::info!("torrent adı bytes: {:?}", r.info.name.as_ref());
    })?;

    // İstatistik task'i — 2 sn'de bir durum bas.
    tokio::spawn({
        let h = handle.clone();
        async move {
            loop {
                tokio::time::sleep(Duration::from_secs(2)).await;
                let s = h.stats();
                match &s.live {
                    Some(live) => {
                        tracing::info!(
                            "durum: {}/{} byte, down={:?}, up={:?}, finished={}",
                            s.progress_bytes,
                            s.total_bytes,
                            live.download_speed,
                            live.upload_speed,
                            s.finished
                        );
                    }
                    None => {
                        tracing::info!(
                            "durum: {}/{} byte, finished={}",
                            s.progress_bytes,
                            s.total_bytes,
                            s.finished
                        );
                    }
                }
            }
        }
    });

    // Tamamlanana kadar bekle (timeout: 15 dk).
    let timeout = tokio::time::timeout(Duration::from_secs(900), handle.wait_until_completed()).await;
    match timeout {
        Ok(_) => tracing::info!("TORRENT İNDİRİLDİ ✅"),
        Err(_) => tracing::warn!("zaman aşımı — indirme tamamlanamadı"),
    }

    Ok(())
}
//! Faz 10b — librqbit `LibrqbitEngine::download_torrent` canlı doğrulaması.
//!
//! Küçük bir public torrent (`<engine>` üzerinden) indirir, dosyayı çözer ve
//! hedef yola link/kopyalar. Python `download_torrent_via_qbittorrent` akışının
//! librqbit karşılığı — `output_dir` içine iner, `dest_path`'e sonuç yazılır.
//! Kullanım: `cargo run --release -p manager-torrent --example live_torrent <output_dir> <dest_path>`
//!
//! Bu örnek Faz 10b'nin engine-seviyesindeki fizibilite kanıtıdır.

use std::time::Duration;

use manager_bridge::TorrentBackend;
use manager_torrent::LibrqbitEngine;

/// Sintel (public domain film) — küçük, güvenilir, iyi seed'li.
const TORRENT_URL: &str = "https://webtorrent.io/torrents/sintel.torrent";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()))
        .init();

    let output_dir = std::env::args()
        .nth(1)
        .expect("ilk argüman çıktı dizini olmalı");
    let dest_path = std::env::args()
        .nth(2)
        .expect("ikinci argüman hedef dosya yolu olmalı");

    let engine = LibrqbitEngine::new(
        output_dir.into(),
        String::new(),
        String::new(),
    );
    tracing::info!("librqbit engine kuruldu (aarch64 Linux), torrent: {TORRENT_URL}");

    let result = tokio::time::timeout(
        Duration::from_secs(900),
        engine.download_torrent(TORRENT_URL, dest_path.as_ref(), None),
    )
    .await;

    match result {
        Ok(Ok(src)) => tracing::info!("TORRENT İNDİRİLDİ ✅ kaynak={}", src.display()),
        Ok(Err(e)) => tracing::error!("indirme hatası: {e}"),
        Err(_) => tracing::warn!("zaman aşımı — indirme tamamlanamadı"),
    }

    engine.shutdown().await;
    Ok(())
}
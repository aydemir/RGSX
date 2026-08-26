//! Gap-4 4a — HTTP stream indirme (`.part` yazma, Range resume, progress, cancel).
//!
//! Python `network/http_download.py::_stream_response_to_path` + `.part` yardımcılarının
//! (`_http_part_path`, `_http_resume_offset`) Rust karşılığı. İlk ~4KB ilk tespit
//! tamponu olarak toplanır (challenge/HTML/arşiv imza guard'ları için); kalanı doğrudan
//! diske akar — büyük dosya belleğe alınmaz.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::io::AsyncWriteExt;

use super::guards::parse_content_range_total;

/// `dest_path`'in `.part` uzantılı kardeşi (Python `_http_part_path`).
pub fn part_path_for(dest_path: &Path) -> PathBuf {
    let mut name = dest_path.as_os_str().to_os_string();
    name.push(".part");
    PathBuf::from(name)
}

/// `.part` dosyasının mevcut boyutu (yoksa 0 → resume yok). Python
/// `_http_resume_offset`.
pub fn resume_offset(dest_path: &Path) -> u64 {
    let part = part_path_for(dest_path);
    match std::fs::metadata(&part) {
        Ok(m) if m.len() > 0 => m.len(),
        _ => 0,
    }
}

/// Stream sonucu (Python `_stream_response_to_path` dönüş dict'i karşılığı).
#[derive(Debug, Clone, Default)]
pub struct StreamResult {
    pub total_size: u64,
    pub downloaded: u64,
    pub canceled: bool,
}

/// Cancellation token — `HttpDownloader::cancel` üzerinden set edilir.
#[derive(Debug, Clone, Default)]
pub struct CancelFlag {
    inner: Arc<std::sync::atomic::AtomicBool>,
}

impl CancelFlag {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
    pub fn set(&self) {
        self.inner.store(true, std::sync::atomic::Ordering::SeqCst);
    }
    pub fn is_set(&self) -> bool {
        self.inner.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// Progress callback — `(downloaded, total)` (Python progress_queue eşleniği).
pub type ProgressCb = dyn Fn(u64, u64) + Send + Sync + 'static;

/// Tespit tamponu üst sınırı — challenge/HTML guard'ları ilk 4KB ile çalışır
/// (Python `_is_browser_challenge_response` 4000, `_looks_like_html_or_challenge` 2048).
pub const DETECT_BUF_MAX: usize = 4096;

/// `reqwest::Response`'tan stream'e yazma. `resume_offset > 0` ise `.part` dosya
/// `append` modunda açılır (206 Range yanıtı beklenir — caller kontrol eder);
/// aksi halde `truncate`.
///
/// Return: `(StreamResult, detect_buf)` — `detect_buf` yalnız ilk yazımda
/// (resume yoksa) ilk ~4KB'ı içerir; resume'da boş (Python guard'ları tam dosyayı
/// tekrar okur; burada resume zaten geçerli `.part` varlığı demektir).
pub async fn download_stream_async(
    response: reqwest::Response,
    dest_path: &Path,
    resume_offset: u64,
    cancel: Option<&CancelFlag>,
    on_progress: Option<Arc<ProgressCb>>,
) -> Result<(StreamResult, Vec<u8>), std::io::Error> {
    let part_path = part_path_for(dest_path);
    if let Some(parent) = part_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let is_range = resume_offset > 0 && response.status() == reqwest::StatusCode::PARTIAL_CONTENT;
    let content_range_total = response
        .headers()
        .get(reqwest::header::CONTENT_RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(parse_content_range_total);
    let content_length = response
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    let total_size = content_range_total
        .filter(|t| *t > 0)
        .or_else(|| {
            if content_length > 0 {
                Some(content_length + if is_range { resume_offset } else { 0 })
            } else {
                None
            }
        })
        .unwrap_or(0);

    let mut downloaded = if is_range { resume_offset } else { 0 };
    let mut detect: Vec<u8> = Vec::with_capacity(DETECT_BUF_MAX.min(4096));

    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(is_range)
        .truncate(!is_range)
        .open(&part_path)
        .await?;

    let mut stream = response.bytes_stream();
    while let Some(chunk) = futures_util::StreamExt::next(&mut stream).await {
        if let Some(c) = cancel {
            if c.is_set() {
                let _ = file.flush().await;
                let _ = tokio::fs::remove_file(&part_path).await;
                return Ok((
                    StreamResult {
                        total_size,
                        downloaded,
                        canceled: true,
                    },
                    detect,
                ));
            }
        }
        match chunk {
            Ok(bytes) => {
                let size = bytes.len() as u64;
                if detect.len() < DETECT_BUF_MAX {
                    let take = (bytes.len()).min(DETECT_BUF_MAX - detect.len());
                    detect.extend_from_slice(&bytes[..take]);
                }
                if let Err(e) = file.write_all(&bytes).await {
                    return Err(e);
                }
                downloaded += size;
                if let Some(cb) = &on_progress {
                    cb(downloaded, total_size);
                }
            }
            Err(e) => return Err(std::io::Error::other(e.to_string())),
        }
    }
    file.flush().await?;
    file.sync_all().await?;

    Ok((
        StreamResult {
            total_size,
            downloaded,
            canceled: false,
        },
        detect,
    ))
}

/// `.part`'ı nihai hedefe taşır (Python `os.replace(part, dest)` eşleniği).
/// İndirilen byte 0 ise `.part` silinir (boş indirme koruması).
pub async fn finalize_part(dest_path: &Path, downloaded: u64) -> std::io::Result<()> {
    let part_path = part_path_for(dest_path);
    if downloaded > 0 {
        tokio::fs::rename(&part_path, dest_path).await?;
    } else if part_path.exists() {
        let _ = tokio::fs::remove_file(&part_path).await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn part_path_and_resume() {
        let dest = Path::new("/tmp/x/game.zip");
        assert_eq!(part_path_for(dest), PathBuf::from("/tmp/x/game.zip.part"));
        // Mevcut olmayan .part → resume 0.
        assert_eq!(resume_offset(dest), 0);
    }

    #[test]
    fn cancel_flag() {
        let f = CancelFlag::new();
        assert!(!f.is_set());
        f.set();
        assert!(f.is_set());
    }
}

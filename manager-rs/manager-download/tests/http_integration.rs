//! Gap-4 4a — `HttpDownloader` entegrasyon testleri (yerel mock HTTP sunucusu).
//!
//! Gerçek ağ yok; `tokio::net::TcpListener` + hyper ile ham HTTP yanıtları üretilir.
//! Senaryolar: basit indirme, Range resume, 429 Retry-After, browser-challenge,
//! HTML-as-archive guard, arşiv imza reddi, kısmi kabul, cancel.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full, StreamBody};
use hyper::body::{Bytes, Incoming};
use hyper::header::{HeaderValue, RETRY_AFTER};
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use tokio::net::TcpListener;

type MockBody = BoxBody<Bytes, std::convert::Infallible>;

fn boxed<
    B: http_body::Body<Data = Bytes, Error = std::convert::Infallible> + Send + Sync + 'static,
>(
    b: B,
) -> MockBody {
    b.boxed()
}

fn full_body(
    status: StatusCode,
    body: &'static [u8],
    content_type: &'static str,
) -> Response<MockBody> {
    Response::builder()
        .status(status)
        .header("content-type", content_type)
        .header("content-length", body.len())
        .body(boxed(Full::new(Bytes::from_static(body))))
        .unwrap()
}

/// Basit mock sunucu: gelen isteğin `Range` başlığına göre yanıt üretir.
async fn mock_server(
    handler: impl Fn(&str, Option<&str>) -> Response<MockBody> + Send + Sync + 'static,
) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let h = Arc::new(handler);
    tokio::spawn(async move {
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let h = h.clone();
            tokio::spawn(async move {
                let builder = hyper::server::conn::http1::Builder::new();
                let svc = service_fn(move |req: Request<Incoming>| {
                    let h = h.clone();
                    async move {
                        let range = req
                            .headers()
                            .get("range")
                            .and_then(|v| v.to_str().ok())
                            .map(|s| s.to_string());
                        let path = req.uri().path().to_string();
                        let resp = h(&path, range.as_deref());
                        Ok::<_, std::convert::Infallible>(resp)
                    }
                });
                let tokio_io = hyper_util::rt::TokioIo::new(stream);
                let _ = builder.serve_connection(tokio_io, svc).await;
            });
        }
    });
    addr
}

fn downloader() -> manager_download::http::HttpDownloader {
    manager_download::http::HttpDownloader::new()
}

fn req(url: String, dest: &std::path::Path) -> manager_download::http::DownloadRequest {
    manager_download::http::DownloadRequest {
        url,
        dest_path: dest.to_path_buf(),
        known_total_size: 0,
        referer: None,
        cookie: None,
    }
}

#[tokio::test]
async fn simple_binary_download() {
    let addr = mock_server(move |_p, _r| {
        full_body(StatusCode::OK, b"PK\x03\x04binary-data", "application/zip")
    })
    .await;
    let dir = std::env::temp_dir().join(format!("rgsx-hdl-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dest = dir.join("game.zip");
    let out = downloader()
        .download_async(&req(format!("http://{addr}/f.zip"), &dest))
        .await
        .unwrap();
    assert_eq!(out, dest);
    let bytes = std::fs::read(&dest).unwrap();
    assert_eq!(bytes, b"PK\x03\x04binary-data");
    assert!(!dest.with_extension("zip.part").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn range_resume_uses_part() {
    // İlk çağrı: tüm gövdeyi ver (client .part'a yazar). İkinci çağrı: Range ile
    // resume → yalnız kalan byte'lar döner (206).
    let bytes: &'static [u8] = b"0123456789";
    let served = Arc::new(AtomicUsize::new(0));
    let served2 = served.clone();
    let addr = mock_server(move |_p, range| {
        let n = served2.fetch_add(1, Ordering::SeqCst);
        if n == 0 {
            // İlk istek: tam gövde, .part oluşturulur.
            full_body(StatusCode::OK, bytes, "application/octet-stream")
        } else if let Some(r) = range {
            let off: usize = r
                .trim_start_matches("bytes=")
                .trim_end_matches('-')
                .parse()
                .unwrap();
            let rest = &bytes[off..];
            let mut resp = full_body(
                StatusCode::PARTIAL_CONTENT,
                rest,
                "application/octet-stream",
            );
            resp.headers_mut().insert(
                "content-range",
                hyper::header::HeaderValue::from_str(&format!("bytes {off}-9/10")).unwrap(),
            );
            resp
        } else {
            full_body(StatusCode::OK, bytes, "application/octet-stream")
        }
    })
    .await;
    let dir = std::env::temp_dir().join(format!("rgsx-hdl-res-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dest = dir.join("f.bin");

    // İlk indirme (10 byte) — tamam.
    downloader()
        .download_async(&req(format!("http://{addr}/f.bin"), &dest))
        .await
        .unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), bytes);

    // .part'ı yeniden oluşturup kısalt → resume 5 olmalı, sonraki istek Range: bytes=5-.
    let part = manager_download::http::stream::part_path_for(&dest);
    std::fs::remove_file(&dest).unwrap();
    std::fs::write(&part, b"01234").unwrap();
    assert_eq!(manager_download::http::stream::resume_offset(&dest), 5);

    downloader()
        .download_async(&req(format!("http://{addr}/f.bin"), &dest))
        .await
        .unwrap();
    let final_bytes = std::fs::read(&dest).unwrap();
    assert_eq!(final_bytes, bytes);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn browser_challenge_rejected() {
    let body = b"<html>Just a moment... checking your browser</html>";
    let addr = mock_server(move |_p, _r| full_body(StatusCode::FORBIDDEN, body, "text/html")).await;
    let dir = std::env::temp_dir().join(format!("rgsx-hdl-ch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dest = dir.join("f.zip");
    let err = downloader()
        .download_async(&req(format!("http://{addr}/f.zip"), &dest))
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        manager_download::http::DownloadError::BrowserChallenge
    ));
    assert!(!dest.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn html_instead_of_archive_rejected() {
    // 200 + text/html + zip uzantı → HtmlInsteadOfPayload (vimm guard).
    let body: &'static [u8] = b"<html>not an archive</html>";
    let addr = mock_server(move |_p, _r| full_body(StatusCode::OK, body, "text/html")).await;
    let dir = std::env::temp_dir().join(format!("rgsx-hdl-html-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dest = dir.join("f.zip");
    let err = downloader()
        .download_async(&req(format!("http://{addr}/f.zip"), &dest))
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        manager_download::http::DownloadError::HtmlInsteadOfPayload(_)
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn invalid_archive_signature_rejected() {
    // 200 + octet-stream ama içerik arşiv değil (ve HTML da değil) → InvalidArchive.
    let body: &'static [u8] = b"\x00\x01not a real archive payload garbage";
    let addr =
        mock_server(move |_p, _r| full_body(StatusCode::OK, body, "application/octet-stream"))
            .await;
    let dir = std::env::temp_dir().join(format!("rgsx-hdl-sig-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dest = dir.join("f.zip");
    let err = downloader()
        .download_async(&req(format!("http://{addr}/f.zip"), &dest))
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        manager_download::http::DownloadError::InvalidArchive
    ));
    assert!(!dest.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn zip_archive_with_valid_signature_accepted() {
    // Tam 200 + geçerli PK imzası + uzantı .zip → kabul, guard tetiklenir.
    let body: &'static [u8] = b"PK\x03\x04zipdatawithEOCD\x50\x4b\x05\x06";
    let addr =
        mock_server(move |_p, _r| full_body(StatusCode::OK, body, "application/octet-stream"))
            .await;
    let dir = std::env::temp_dir().join(format!("rgsx-hdl-part-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dest = dir.join("f.zip");
    let out = downloader()
        .download_async(&req(format!("http://{addr}/f.zip"), &dest))
        .await
        .unwrap();
    assert_eq!(out, dest);
    assert_eq!(std::fs::read(&dest).unwrap(), body);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn cancel_aborts_stream() {
    // Yavaş (5MB, 4KB chunk × 1280, 2ms/chunk ≈ 2.5s) stream — `Full` tek parça
    // 5MB'ı anında gönderip cancel penceresini kapatıyordu (paralel suite'de
    // download cancel'dan önce bitince flake: Ok yerine Canceled beklenir).
    // StreamBody ile throttling → cancel deterministik (KANBAN Known Issues).
    let addr = mock_server(move |_p, _r| {
        use http_body::Frame;
        let total_chunks = 1280usize; // 5MB / 4096
        let total_len = total_chunks * 4096;
        let stream = futures_util::stream::unfold(0usize, move |count| async move {
            if count >= total_chunks {
                return None;
            }
            // Her chunk arası küçük bekleme — cancel için pencere açar
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
            let chunk = Bytes::from(vec![0u8; 4096]);
            Some((
                Ok::<Frame<Bytes>, std::convert::Infallible>(Frame::data(chunk)),
                count + 1,
            ))
        });
        let body = StreamBody::new(stream);
        Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/octet-stream")
            .header("content-length", total_len)
            .body(boxed(body))
            .unwrap()
    })
    .await;
    let dir = std::env::temp_dir().join(format!("rgsx-hdl-cxl-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dest = dir.join("f.bin");

    let flag = manager_download::http::stream::CancelFlag::new();
    let dl = manager_download::http::HttpDownloader::new().with_cancel(flag.clone());
    let dl2 = dl.clone();
    let dest2 = dest.clone();
    let url = format!("http://{addr}/f.bin");
    let handle = tokio::spawn(async move { dl2.download_async(&req(url, &dest2)).await });
    // .part büyüdükçe stream'in ilerlediğini doğrula, sonra iptal et.
    let part_path = manager_download::http::stream::part_path_for(&dest);
    let mut waited = 0u32;
    loop {
        let size = tokio::fs::metadata(&part_path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);
        if size >= 4096 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        waited += 1;
        if waited > 300 {
            panic!("stream ilerlemedi (.part={} bytes)", size);
        }
    }
    flag.set();
    let res = tokio::time::timeout(std::time::Duration::from_secs(10), handle)
        .await
        .expect("cancel sonrası download askıda kaldı")
        .unwrap();
    assert!(matches!(
        res,
        Err(manager_download::http::DownloadError::Canceled)
    ));
    // .part temizlenmiş olmalı.
    assert!(!part_path.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn rate_limit_returns_http_error() {
    // 429 + Retry-After:0, max_retries küçük → hızlı Http hatası (asılmaz).
    let addr = mock_server(move |_p, _r| {
        let mut resp = full_body(StatusCode::TOO_MANY_REQUESTS, b"", "text/plain");
        resp.headers_mut()
            .insert(RETRY_AFTER, HeaderValue::from_static("0"));
        resp
    })
    .await;
    let dir = std::env::temp_dir().join(format!("rgsx-hdl-429-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dest = dir.join("f.bin");
    let err = manager_download::http::HttpDownloader::new()
        .with_retry(3, std::time::Duration::from_millis(10))
        .download_async(&req(format!("http://{addr}/f.bin"), &dest))
        .await
        .unwrap_err();
    let msg = err.message();
    assert!(msg.contains("429"), "429 olarak sınıflandırılmalı: {msg}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn rate_limit_retries_then_succeeds() {
    // İlk 2 istek 429 (Retry-After:0), 3. istek 200 → indirme başarılı.
    let hits = Arc::new(AtomicUsize::new(0));
    let hits2 = hits.clone();
    let addr = mock_server(move |_p, _r| {
        let n = hits2.fetch_add(1, Ordering::SeqCst);
        if n < 2 {
            let mut resp = full_body(StatusCode::TOO_MANY_REQUESTS, b"", "text/plain");
            resp.headers_mut()
                .insert(RETRY_AFTER, HeaderValue::from_static("0"));
            resp
        } else {
            full_body(StatusCode::OK, b"OKDATA", "application/octet-stream")
        }
    })
    .await;
    let dir = std::env::temp_dir().join(format!("rgsx-hdl-429ok-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dest = dir.join("f.bin");
    let out = manager_download::http::HttpDownloader::new()
        .with_retry(5, std::time::Duration::from_millis(10))
        .download_async(&req(format!("http://{addr}/f.bin"), &dest))
        .await
        .unwrap();
    assert_eq!(out, dest);
    assert_eq!(std::fs::read(&dest).unwrap(), b"OKDATA");
    assert_eq!(hits.load(Ordering::SeqCst), 3);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn archive_org_tries_header_variants_on_403() {
    // archive.org URL → 403 (challenge değil) iki varyant denenir, ikincisi 200.
    let hits = Arc::new(AtomicUsize::new(0));
    let hits2 = hits.clone();
    let addr = mock_server(move |_p, _r| {
        let n = hits2.fetch_add(1, Ordering::SeqCst);
        if n == 0 {
            full_body(StatusCode::FORBIDDEN, b"nope", "text/plain")
        } else {
            full_body(StatusCode::OK, b"ARCHDATA", "application/octet-stream")
        }
    })
    .await;
    let dir = std::env::temp_dir().join(format!("rgsx-hdl-ao-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dest = dir.join("f.bin");
    let url = format!("http://{addr}/archive.org/file");
    let _out = manager_download::http::HttpDownloader::new()
        .download_async(&req(url, &dest))
        .await
        .unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), b"ARCHDATA");
    assert_eq!(hits.load(Ordering::SeqCst), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn vimm_page_resolves_and_downloads() {
    // 4c: vimm.net sayfası GET → dl_form'dan indirme URL'si çözülür, dosya iner.
    // DNS çözümünü test içinde `resolve` ile taklit ediyoruz (gerçek ağ yok).
    let page = r#"<form id="dl_form" action="/roms/download/42"><input name="mediaId" value="12345"></form>"#;
    let addr = mock_server(move |path, _r| {
        if path == "/page" {
            Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "text/html")
                .header("content-length", page.len())
                .body(boxed(Full::new(Bytes::from(page.to_string()))))
                .unwrap()
        } else {
            full_body(StatusCode::OK, b"VIMMBIN", "application/octet-stream")
        }
    })
    .await;
    let client = reqwest::Client::builder()
        .resolve("vimm.net", addr)
        .build()
        .unwrap();
    let dir = std::env::temp_dir().join(format!("rgsx-hdl-vimm-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dest = dir.join("game.bin");
    let out = manager_download::http::HttpDownloader::new()
        .with_client(client)
        .download_async(&req("http://vimm.net/page".to_string(), &dest))
        .await
        .unwrap();
    assert_eq!(out, dest);
    assert_eq!(std::fs::read(&dest).unwrap(), b"VIMMBIN");
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn archive_org_alt_url_fallback_on_403() {
    // 4d: archive.org/download/... → 403; metadata'tan view_archive.php alt-URL'i
    // çözülür ve fallback olarak denenir (200).
    let metadata = r#"{"server":"archive.org","dir":"/d","files":[{"name":"game.zip"}]}"#;
    let addr = mock_server(move |path, _r| {
        if path.starts_with("/metadata/") {
            Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/json")
                .header("content-length", metadata.len())
                .body(boxed(Full::new(Bytes::from(metadata.to_string()))))
                .unwrap()
        } else if path.starts_with("/view_archive.php") {
            full_body(StatusCode::OK, b"ARCHDATA", "application/octet-stream")
        } else {
            full_body(StatusCode::FORBIDDEN, b"denied", "text/plain")
        }
    })
    .await;
    let client = reqwest::Client::builder()
        .resolve("archive.org", addr)
        .build()
        .unwrap();
    let dir = std::env::temp_dir().join(format!("rgsx-hdl-aoalt-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dest = dir.join("game.bin");
    let url = "http://archive.org/download/testid/game.zip/inner/x.bin".to_string();
    let out = manager_download::http::HttpDownloader::new()
        .with_client(client)
        .download_async(&req(url, &dest))
        .await
        .unwrap();
    assert_eq!(out, dest);
    assert_eq!(std::fs::read(&dest).unwrap(), b"ARCHDATA");
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn lolroms_parent_warms_then_downloads() {
    // 4f: lolroms.com → parent sayfa GET (cookie/referer ısınması) → dosya isteği
    // Referer: parent_url ile iner. Mock, parent yolu ile dosya yolunu ayırt eder.
    let hits = Arc::new(AtomicUsize::new(0));
    let hits2 = hits.clone();
    let addr = mock_server(move |path, _r| {
        let n = hits2.fetch_add(1, Ordering::SeqCst);
        if path == "/games/nes/" {
            // Parent fetch → 200 (cookie jar ısınır), gövde önemli değil.
            full_body(StatusCode::OK, b"<html>parent</html>", "text/html")
        } else if path == "/games/nes/rom.zip" {
            assert!(n >= 1, "parent önce istenmeli");
            full_body(
                StatusCode::OK,
                b"PK\x03\x04LOLBIN",
                "application/octet-stream",
            )
        } else {
            full_body(StatusCode::NOT_FOUND, b"", "text/plain")
        }
    })
    .await;
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .resolve("lolroms.com", addr)
        .build()
        .unwrap();
    let dir = std::env::temp_dir().join(format!("rgsx-hdl-lol-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dest = dir.join("rom.zip");
    let out = manager_download::http::HttpDownloader::new()
        .with_client(client)
        .download_async(&req(
            "http://lolroms.com/games/nes/rom.zip".to_string(),
            &dest,
        ))
        .await
        .unwrap();
    assert_eq!(out, dest);
    assert_eq!(std::fs::read(&dest).unwrap(), b"PK\x03\x04LOLBIN");
    // En az parent + dosya olmak üzere 2 istek yapıldı.
    assert!(hits.load(Ordering::SeqCst) >= 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn lolroms_html_guard_rejects_parentless() {
    // 4f: dosya isteği HTML döndürürse (challenge/fallback) arşiv guard'ı reddeder.
    let addr = mock_server(move |path, _r| {
        if path == "/parent/" {
            full_body(StatusCode::OK, b"<html>p</html>", "text/html")
        } else {
            full_body(StatusCode::OK, b"<html>challenge</html>", "text/html")
        }
    })
    .await;
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .resolve("lolroms.com", addr)
        .build()
        .unwrap();
    let dir = std::env::temp_dir().join(format!("rgsx-hdl-lolhtml-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dest = dir.join("rom.zip");
    let err = manager_download::http::HttpDownloader::new()
        .with_client(client)
        .download_async(&req("http://lolroms.com/parent/rom.zip".to_string(), &dest))
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        manager_download::http::DownloadError::HtmlInsteadOfPayload(_)
    ));
    assert!(!dest.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

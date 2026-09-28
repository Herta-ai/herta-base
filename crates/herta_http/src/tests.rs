use super::*;
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

struct Dns {
    answers: Mutex<Vec<Vec<IpAddr>>>,
    calls: AtomicUsize,
}
#[async_trait]
impl Resolver for Dns {
    async fn resolve(&self, _: &str, port: u16) -> HbResult<Vec<SocketAddr>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut answers = self.answers.lock().unwrap();
        let ips = if answers.len() > 1 {
            answers.remove(0)
        } else {
            answers[0].clone()
        };
        Ok(ips
            .into_iter()
            .map(|ip| SocketAddr::new(ip, port))
            .collect())
    }
}
fn dns(answers: &[&[&str]]) -> Arc<Dns> {
    Arc::new(Dns {
        answers: Mutex::new(
            answers
                .iter()
                .map(|answer| answer.iter().map(|ip| ip.parse().unwrap()).collect())
                .collect(),
        ),
        calls: AtomicUsize::new(0),
    })
}
fn service(origins: &[String], resolver: Arc<Dns>) -> HttpService {
    let mut service = HttpService::new(JsHttpConfig {
        enabled: true,
        allowlist: origins.to_vec(),
        ..Default::default()
    })
    .unwrap();
    service.resolver = resolver;
    service.loopback = true;
    service
}
fn request(url: String) -> HttpRequest {
    HttpRequest {
        url,
        method: "GET".into(),
        headers: BTreeMap::new(),
        body: None,
        timeout_ms: None,
    }
}
async fn read_request(stream: &mut (impl tokio::io::AsyncRead + Unpin)) -> String {
    let mut request = Vec::new();
    loop {
        let mut byte = [0];
        if stream.read(&mut byte).await.unwrap_or(0) == 0 {
            break;
        }
        request.push(byte[0]);
        if request.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let head = String::from_utf8(request.clone()).unwrap();
    let length: usize = head
        .lines()
        .find_map(|line| {
            line.to_lowercase()
                .strip_prefix("content-length: ")
                .map(str::to_owned)
        })
        .map(|value| value.parse().unwrap())
        .unwrap_or(0);
    let mut body = vec![0; length];
    stream.read_exact(&mut body).await.unwrap();
    request.extend(body);
    String::from_utf8(request).unwrap()
}
async fn receiver(
    responses: Vec<(String, Duration)>,
) -> (u16, tokio::task::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    (
        port,
        tokio::spawn(async move {
            let mut received = Vec::new();
            for (response, delay) in responses {
                let (mut stream, _) = listener.accept().await.unwrap();
                received.push(read_request(&mut stream).await);
                tokio::time::sleep(delay).await;
                let _ = stream.write_all(response.as_bytes()).await;
            }
            received
        }),
    )
}
fn reply(body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[test]
fn only_global_unicast_addresses_are_allowed() {
    for ip in [
        "0.0.0.0",
        "10.1.2.3",
        "100.64.0.1",
        "127.0.0.1",
        "169.254.169.254",
        "172.16.0.1",
        "192.168.1.1",
        "192.0.2.1",
        "192.88.99.1",
        "198.18.0.1",
        "198.51.100.1",
        "203.0.113.1",
        "224.0.0.1",
        "255.255.255.255",
        "::",
        "::1",
        "::ffff:127.0.0.1",
        "64:ff9b::a00:1",
        "64:ff9b:1::1",
        "100::1",
        "2001::1",
        "2001:2::1",
        "2001:20::1",
        "2001:db8::1",
        "2002:7f00:1::",
        "3fff::1",
        "fc00::1",
        "fe80::1",
        "ff02::1",
    ] {
        assert!(!public_address(ip.parse().unwrap()), "{ip}");
    }
    for ip in [
        "1.1.1.1",
        "8.8.8.8",
        "93.184.216.34",
        "2606:4700:4700::1111",
        "2001:4860:4860::8888",
    ] {
        assert!(public_address(ip.parse().unwrap()), "{ip}");
    }
}

#[tokio::test]
async fn targets_and_every_dns_answer_are_validated_before_connection() {
    let resolver = dns(&[&["93.184.216.34", "10.0.0.1"]]);
    let mut service = service(
        &[
            "https://upstream.invalid".into(),
            "http://127.0.0.1".into(),
            "http://2130706433".into(),
        ],
        resolver.clone(),
    );
    service.loopback = false;
    for url in [
        "https://upstream.invalid",
        "http://127.0.0.1",
        "http://2130706433",
        "http://upstream.invalid",
        "https://user:password@upstream.invalid",
        "https://@upstream.invalid",
        "https://upstream.invalid/#secret",
        "file:///etc/passwd",
    ] {
        let error = service.send(request(url.into()), 1000).await.unwrap_err();
        assert_eq!(error.error.error_code(), "HB_OUTBOUND_DENIED", "{url}");
        assert_eq!(error.delivery, Delivery::NotSent);
    }
    assert_eq!(resolver.calls.load(Ordering::SeqCst), 1);
    let empty = super::tests::service(&["https://upstream.invalid".into()], dns(&[&[]]));
    assert_eq!(
        empty
            .validate(&request("https://upstream.invalid".into()))
            .await
            .unwrap_err()
            .error_code(),
        "HB_OUTBOUND_DENIED"
    );
}

#[tokio::test]
async fn validated_addresses_are_pinned_and_request_body_and_status_are_preserved() {
    let (port, received) = receiver(vec![("HTTP/1.1 422 Unprocessable Entity\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}".into(), Duration::ZERO)]).await;
    let origin = format!("http://does-not-exist.invalid:{port}");
    let resolver = dns(&[&["127.0.0.1"], &["10.0.0.1"]]);
    let service = service(std::slice::from_ref(&origin), resolver.clone());
    let mut request = request(format!("{origin}/path?value=1"));
    request.method = "POST".into();
    request.body = Some("你好".into());
    request
        .headers
        .insert("content-type".into(), "text/plain".into());
    let response = service.send(request, 1000).await.unwrap();
    assert_eq!(response.status, 422);
    assert!(!response.ok);
    assert_eq!(response.body, "{\"ok\":true}");
    assert_eq!(resolver.calls.load(Ordering::SeqCst), 1);
    let received = received.await.unwrap();
    assert!(received[0].contains(&format!("host: does-not-exist.invalid:{port}")));
    assert!(received[0].ends_with("你好"));
}

#[tokio::test]
async fn redirect_rebinding_and_unlisted_targets_are_denied_without_second_send() {
    for target in ["/next", "http://other.invalid/next"] {
        let (port, received) = receiver(vec![(
            format!("HTTP/1.1 302 Found\r\nLocation: {target}\r\nContent-Length: 0\r\n\r\n"),
            Duration::ZERO,
        )])
        .await;
        let origin = format!("http://upstream.invalid:{port}");
        let resolver = dns(&[&["127.0.0.1"], &["10.0.0.1"]]);
        let service = service(std::slice::from_ref(&origin), resolver);
        let error = service.send(request(origin), 1000).await.unwrap_err();
        assert_eq!(error.error.error_code(), "HB_OUTBOUND_DENIED");
        assert_eq!(error.delivery, Delivery::Unknown);
        assert_eq!(received.await.unwrap().len(), 1);
    }
}

#[tokio::test]
async fn cross_origin_redirect_removes_credentials_and_rewrites_post() {
    let (target_port, target_received) = receiver(vec![(reply("done"), Duration::ZERO)]).await;
    let target = format!("http://target.invalid:{target_port}");
    let (port, received) = receiver(vec![(
        format!("HTTP/1.1 302 Found\r\nLocation: {target}/final\r\nContent-Length: 0\r\n\r\n"),
        Duration::ZERO,
    )])
    .await;
    let origin = format!("http://upstream.invalid:{port}");
    let service = service(&[origin.clone(), target], dns(&[&["127.0.0.1"]]));
    let mut request = request(origin);
    request.method = "POST".into();
    request.body = Some("payload".into());
    for name in ["authorization", "cookie", "x-api-key", "referer"] {
        request.headers.insert(name.into(), "secret".into());
    }
    assert_eq!(service.send(request, 1000).await.unwrap().body, "done");
    assert!(received.await.unwrap()[0].contains("secret"));
    let target_received = target_received.await.unwrap();
    assert!(target_received[0].starts_with("GET /final "));
    assert!(!target_received[0].contains("secret"));
    assert!(!target_received[0].contains("payload"));
}

#[tokio::test]
async fn preserving_redirects_keep_body_and_same_origin_headers_but_obey_hop_limit() {
    for status in [307, 308] {
        let (port, received) = receiver(vec![
            (
                format!(
                    "HTTP/1.1 {status} Redirect\r\nLocation: /next\r\nContent-Length: 0\r\n\r\n"
                ),
                Duration::ZERO,
            ),
            (reply("done"), Duration::ZERO),
        ])
        .await;
        let origin = format!("http://upstream.invalid:{port}");
        let service = service(std::slice::from_ref(&origin), dns(&[&["127.0.0.1"]]));
        let mut request = request(origin);
        request.method = "POST".into();
        request.body = Some("payload".into());
        request
            .headers
            .insert("authorization".into(), "Bearer private".into());
        assert_eq!(service.send(request, 1000).await.unwrap().body, "done");
        let calls = received.await.unwrap();
        assert!(calls[1].starts_with("POST /next "));
        assert!(calls[1].ends_with("payload"));
        assert!(calls[1].contains("Bearer private"));
    }
    let (port, received) = receiver(vec![(
        "HTTP/1.1 302 Found\r\nLocation: /next\r\nContent-Length: 0\r\n\r\n".into(),
        Duration::ZERO,
    )])
    .await;
    let origin = format!("http://upstream.invalid:{port}");
    let mut service = service(std::slice::from_ref(&origin), dns(&[&["127.0.0.1"]]));
    service.config.max_redirects = 0;
    assert_eq!(
        service
            .send(request(origin), 1000)
            .await
            .unwrap_err()
            .error
            .error_code(),
        "HB_OUTBOUND_DENIED"
    );
    assert_eq!(received.await.unwrap().len(), 1);
}

#[tokio::test]
async fn streaming_size_and_total_timeout_do_not_retry() {
    let (port, received) = receiver(vec![(
        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\n1234\r\n4\r\n5678\r\n0\r\n\r\n"
            .into(),
        Duration::ZERO,
    )])
    .await;
    let origin = format!("http://upstream.invalid:{port}");
    let mut service = service(std::slice::from_ref(&origin), dns(&[&["127.0.0.1"]]));
    service.config.max_response_bytes = 6;
    let error = service.send(request(origin), 1000).await.unwrap_err();
    assert_eq!(error.error.error_code(), "HB_PAYLOAD_TOO_LARGE");
    assert_eq!(error.delivery, Delivery::Unknown);
    assert_eq!(received.await.unwrap().len(), 1);
    let (port, received) = receiver(vec![(reply("late"), Duration::from_millis(100))]).await;
    let origin = format!("http://upstream.invalid:{port}");
    let service = super::tests::service(std::slice::from_ref(&origin), dns(&[&["127.0.0.1"]]));
    let error = service.send(request(origin), 50).await.unwrap_err();
    assert_eq!(error.error.error_code(), "HB_HTTP_TIMEOUT");
    assert_eq!(error.delivery, Delivery::Unknown);
    assert_eq!(received.await.unwrap().len(), 1);
}

#[test]
fn environment_proxies_are_ignored() {
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "tests::proxy_child", "--nocapture"])
        .env("HB_HTTP_PROXY_TEST_CHILD", "1")
        .env("HTTP_PROXY", "http://127.0.0.1:1")
        .env("HTTPS_PROXY", "http://127.0.0.1:1")
        .env("ALL_PROXY", "http://127.0.0.1:1")
        .env("NO_PROXY", "")
        .env("http_proxy", "http://127.0.0.1:1")
        .env("https_proxy", "http://127.0.0.1:1")
        .env("all_proxy", "http://127.0.0.1:1")
        .env("no_proxy", "")
        .status()
        .unwrap();
    assert!(status.success());
}
#[tokio::test]
async fn proxy_child() {
    if std::env::var_os("HB_HTTP_PROXY_TEST_CHILD").is_none() {
        return;
    }
    let (port, received) = receiver(vec![(reply("direct"), Duration::ZERO)]).await;
    let origin = format!("http://upstream.invalid:{port}");
    let service = service(std::slice::from_ref(&origin), dns(&[&["127.0.0.1"]]));
    assert_eq!(
        service.send(request(origin), 2000).await.unwrap().body,
        "direct"
    );
    received.await.unwrap();
}

#[tokio::test]
async fn invalid_parameters_never_resolve_or_send() {
    let resolver = dns(&[&["127.0.0.1"]]);
    let mut service = service(&["https://upstream.invalid".into()], resolver.clone());
    for name in [
        "host",
        "content-length",
        "transfer-encoding",
        "connection",
        "proxy-authorization",
        "upgrade",
    ] {
        let mut request = request("https://upstream.invalid".into());
        request.headers.insert(name.into(), "value".into());
        assert_eq!(
            service
                .send(request, 1000)
                .await
                .unwrap_err()
                .error
                .error_code(),
            "HB_VALIDATION_ERROR"
        );
    }
    let mut bad = request("https://upstream.invalid".into());
    bad.headers
        .insert("x-key".into(), "value\r\nInjected: yes".into());
    assert_eq!(
        service
            .send(bad, 1000)
            .await
            .unwrap_err()
            .error
            .error_code(),
        "HB_VALIDATION_ERROR"
    );
    service.config.max_request_bytes = 4;
    assert_eq!(
        service
            .send(request("https://upstream.invalid".into()), 1000)
            .await
            .unwrap_err()
            .error
            .error_code(),
        "HB_PAYLOAD_TOO_LARGE"
    );
    service.config.enabled = false;
    assert_eq!(
        service
            .send(request("invalid".into()), 1000)
            .await
            .unwrap_err()
            .error
            .error_code(),
        "HB_CAPABILITY_DENIED"
    );
    assert_eq!(resolver.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn tls_uses_original_hostname_and_never_downgrades() {
    use tokio_rustls::{
        TlsAcceptor,
        rustls::{self, pki_types::PrivatePkcs8KeyDer},
    };
    let certificate = rcgen::generate_simple_self_signed(vec!["upstream.invalid".into()]).unwrap();
    let root = reqwest::Certificate::from_der(certificate.cert.der()).unwrap();
    let tls = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(
        vec![certificate.cert.der().clone()],
        PrivatePkcs8KeyDer::from(certificate.signing_key.serialize_der()).into(),
    )
    .unwrap();
    let acceptor = TlsAcceptor::from(Arc::new(tls));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let mut received = Vec::new();
        for _ in 0..4 {
            let (stream, _) = listener.accept().await.unwrap();
            if let Ok(mut stream) = acceptor.accept(stream).await {
                let request = read_request(&mut stream).await;
                let response = if request.starts_with("GET /downgrade ") {
                    format!(
                        "HTTP/1.1 302 Found\r\nLocation: http://upstream.invalid:{port}/\r\nContent-Length: 0\r\n\r\n"
                    )
                } else {
                    reply("secure")
                };
                received.push(request);
                let _ = stream.write_all(response.as_bytes()).await;
            }
        }
        received
    });
    let origin = format!("https://upstream.invalid:{port}");
    let wrong = format!("https://wrong.invalid:{port}");
    let mut service = service(
        &[
            origin.clone(),
            wrong.clone(),
            format!("http://upstream.invalid:{port}"),
        ],
        dns(&[&["127.0.0.1"]]),
    );
    // Unknown CA is rejected before any HTTP bytes are sent.
    assert_eq!(
        service
            .send(request(origin.clone()), 2000)
            .await
            .unwrap_err()
            .delivery,
        Delivery::NotSent
    );
    service.roots.push(root);
    assert_eq!(
        service
            .send(request(origin.clone()), 2000)
            .await
            .unwrap()
            .body,
        "secure"
    );
    let error = service.send(request(wrong), 2000).await.unwrap_err();
    assert_eq!(error.error.error_code(), "HB_HTTP_SEND_FAILED");
    assert_eq!(error.delivery, Delivery::NotSent);
    let error = service
        .send(request(format!("{origin}/downgrade")), 2000)
        .await
        .unwrap_err();
    assert_eq!(error.error.error_code(), "HB_OUTBOUND_DENIED");
    assert_eq!(error.delivery, Delivery::Unknown);
    assert_eq!(server.await.unwrap().len(), 2);
}

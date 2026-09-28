//! A local S3 protocol receiver exercises the real object_store S3 adapter,
//! including COPY followed by a rejected DELETE. No cloud credentials are used.
use herta_api::files::FileService;
use herta_core::{S3Config, jsvm::JsFilesConfig};
use herta_db::DbClient;
use herta_storage::{ObjectStoreStorage, Storage};
use serde_json::json;
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

#[derive(Default)]
struct Bucket {
    objects: Mutex<BTreeMap<String, Vec<u8>>>,
    reject_delete: AtomicBool,
    fail_put_ack: AtomicBool,
    list_requests: AtomicUsize,
}
async fn request(stream: tokio::net::TcpStream, bucket: Arc<Bucket>) {
    let (read, mut write) = stream.into_split();
    let mut reader = BufReader::new(read);
    let mut first = String::new();
    reader.read_line(&mut first).await.unwrap();
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap();
    let target = parts.next().unwrap();
    let url = url::Url::parse(&format!("http://fixture{target}")).unwrap();
    let query = url.query_pairs().collect::<BTreeMap<_, _>>();
    let path = target.split('?').next().unwrap();
    let path = percent_encoding::percent_decode_str(path)
        .decode_utf8()
        .unwrap()
        .into_owned();
    let mut headers = BTreeMap::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        if line == "\r\n" || line.is_empty() {
            break;
        }
        let (name, value) = line.split_once(':').unwrap();
        headers.insert(name.to_ascii_lowercase(), value.trim().to_owned());
    }
    assert!(
        headers.contains_key("authorization"),
        "S3 requests must be signed"
    );
    let size = headers
        .get("content-length")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    assert!(size < 8192);
    let mut body = vec![0; size];
    reader.read_exact(&mut body).await.unwrap();
    let (status, bytes, length) = {
        let mut objects = bucket.objects.lock().unwrap();
        match method {
            "GET" if query.contains_key("list-type") => {
                bucket.list_requests.fetch_add(1, Ordering::SeqCst);
                let prefix = query.get("prefix").map(|s| s.as_ref()).unwrap_or("");
                let after = query
                    .get("continuation-token")
                    .or_else(|| query.get("start-after"))
                    .map(|s| s.as_ref())
                    .unwrap_or("");
                let mut found = objects
                    .iter()
                    .filter_map(|(path, bytes)| {
                        let key = path.strip_prefix("/fixture/").unwrap();
                        (key.starts_with(prefix) && key > after).then_some((key, bytes))
                    })
                    .take(3)
                    .collect::<Vec<_>>();
                let more = found.len() > 2;
                found.truncate(2);
                let mut xml = format!("<ListBucketResult><IsTruncated>{more}</IsTruncated>");
                for (key, bytes) in &found {
                    xml.push_str(&format!("<Contents><Key>{key}</Key><LastModified>2026-09-28T00:00:00Z</LastModified><ETag>test-etag</ETag><Size>{}</Size></Contents>", bytes.len()));
                }
                if more {
                    xml.push_str(&format!(
                        "<NextContinuationToken>{}</NextContinuationToken>",
                        found.last().unwrap().0
                    ));
                }
                xml.push_str("</ListBucketResult>");
                let length = xml.len();
                ("200 OK", xml.into_bytes(), length)
            }
            "POST" => {
                if bucket.reject_delete.load(Ordering::SeqCst) {
                    ("403 Forbidden", Vec::new(), 0)
                } else {
                    let xml = String::from_utf8(body).unwrap();
                    let mut response = String::from("<DeleteResult>");
                    for key in xml
                        .split("<Key>")
                        .skip(1)
                        .map(|part| part.split("</Key>").next().unwrap())
                    {
                        objects.remove(&format!("/fixture/{key}"));
                        response.push_str(&format!("<Deleted><Key>{key}</Key></Deleted>"));
                    }
                    response.push_str("</DeleteResult>");
                    let length = response.len();
                    ("200 OK", response.into_bytes(), length)
                }
            }
            "PUT" => {
                if let Some(source) = headers.get("x-amz-copy-source") {
                    let source = percent_encoding::percent_decode_str(source)
                        .decode_utf8()
                        .unwrap();
                    let source = format!("/{}", source.trim_start_matches('/'));
                    let bytes = objects.get(&source).expect("copy source").clone();
                    objects.insert(path.clone(), bytes);
                    let xml=b"<CopyObjectResult><LastModified>2026-09-28T00:00:00Z</LastModified><ETag>\"test-etag\"</ETag></CopyObjectResult>".to_vec();
                    let length = xml.len();
                    ("200 OK", xml, length)
                } else {
                    objects.insert(path.clone(), body);
                    if bucket.fail_put_ack.load(Ordering::SeqCst) {
                        ("403 Forbidden", Vec::new(), 0)
                    } else {
                        ("200 OK", Vec::new(), 0)
                    }
                }
            }
            "GET" | "HEAD" => match objects.get(&path) {
                Some(bytes) => (
                    "200 OK",
                    if method == "HEAD" {
                        Vec::new()
                    } else {
                        bytes.clone()
                    },
                    bytes.len(),
                ),
                None => ("404 Not Found", Vec::new(), 0),
            },
            "DELETE" if bucket.reject_delete.load(Ordering::SeqCst) => {
                ("403 Forbidden", Vec::new(), 0)
            }
            "DELETE" => {
                objects.remove(&path);
                ("204 No Content", Vec::new(), 0)
            }
            _ => panic!("unexpected S3 request {first}"),
        }
    };
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {length}\r\nETag: \"test-etag\"\r\nLast-Modified: Mon, 28 Sep 2026 00:00:00 GMT\r\nConnection: close\r\n\r\n"
    );
    write.write_all(header.as_bytes()).await.unwrap();
    write.write_all(&bytes).await.unwrap();
}

#[tokio::test]
async fn s3_copy_delete_partial_failure_and_unknown_put_keep_the_same_journal_semantics() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let bucket = Arc::new(Bucket::default());
    let receiver = bucket.clone();
    let task = tokio::spawn(async move {
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            tokio::spawn(request(stream, receiver.clone()));
        }
    });
    let storage = Arc::new(
        ObjectStoreStorage::s3(&S3Config {
            endpoint: Some(endpoint),
            bucket: "fixture".into(),
            region: "us-east-1".into(),
            allow_http: true,
            access_key: Some("test-access".into()),
            secret_key: Some("test-secret".into()),
            ..Default::default()
        })
        .unwrap(),
    );
    let db = DbClient::memory().await.unwrap();
    let files = FileService::new(
        db.clone(),
        storage.clone(),
        JsFilesConfig {
            enabled: true,
            quota_bytes: 12,
            max_file_bytes: 12,
            ..Default::default()
        },
    );
    let source = files
        .call("files.write", json!({"key":"source","text":"value"}))
        .await
        .unwrap();
    bucket.reject_delete.store(true, Ordering::SeqCst);
    let error = files
        .call(
            "files.move",
            json!({"source":"source","destination":"destination"}),
        )
        .await
        .unwrap_err();
    assert_eq!(error.error_code(), "HB_FILE_MOVE_PARTIAL");
    assert_eq!(
        files
            .call("files.readText", json!({"key":"destination"}))
            .await
            .unwrap(),
        "value"
    );
    assert!(
        files
            .call("files.write", json!({"key":"over-quota","text":"123"}))
            .await
            .is_err()
    );
    bucket.reject_delete.store(false, Ordering::SeqCst);
    files.reconcile().await.unwrap();
    assert_eq!(
        files
            .call("files.exists", json!({"key":"source"}))
            .await
            .unwrap(),
        false
    );
    let destination = files
        .call("files.stat", json!({"key":"destination"}))
        .await
        .unwrap();
    assert_ne!(source["version"], destination["version"]);
    bucket.fail_put_ack.store(true, Ordering::SeqCst);
    assert!(
        files
            .call("files.write", json!({"key":"unknown","text":"1234567"}))
            .await
            .is_err()
    );
    assert_eq!(bucket.objects.lock().unwrap().len(), 2);
    assert!(
        files
            .call("files.write", json!({"key":"over-quota","text":"x"}))
            .await
            .is_err()
    );
    files.reconcile().await.unwrap();
    assert_eq!(bucket.objects.lock().unwrap().len(), 1);
    assert_eq!(
        files
            .call("files.readText", json!({"key":"destination"}))
            .await
            .unwrap(),
        "value"
    );
    // Provider pages (two objects here) and our smaller keyset pages compose, even
    // when each page's objects are deleted before requesting the next page.
    bucket.fail_put_ack.store(false, Ordering::SeqCst);
    for index in [5, 1, 4, 0, 3, 2] {
        storage
            .put_bytes(&format!("pages/{index}"), bytes::Bytes::from_static(b"x"))
            .await
            .unwrap();
    }
    let mut cursor = None;
    let mut keys = Vec::new();
    loop {
        let page = storage
            .list_page("pages", 1, cursor.as_deref())
            .await
            .unwrap();
        for (key, _) in page.items {
            storage.delete(&key).await.unwrap();
            keys.push(key);
        }
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(
        keys,
        (0..6).map(|i| format!("pages/{i}")).collect::<Vec<_>>()
    );
    assert!(bucket.list_requests.load(Ordering::SeqCst) > 6);
    task.abort();
}

//! Restricted outbound HTTP. Every hop gets its own pinned, proxy-free client.
use async_trait::async_trait;
use futures_util::StreamExt;
use herta_core::{
    HbError, HbResult, JsErrorKind,
    http::{HttpRequest, HttpResponse},
    jsvm::{JsHttpConfig, exact_origin, has_url_userinfo},
};
use reqwest::{
    Client, Method,
    header::{HeaderMap, HeaderName, HeaderValue, LOCATION},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use url::{Host, Url};

const MAX_ADDRESSES: usize = 64;
const MAX_HEADERS: usize = 64;
const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_URL_BYTES: usize = 8192;

/// A failed wait is never evidence that the receiver did not accept the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    NotSent,
    Unknown,
}

#[derive(Debug)]
pub struct HttpFailure {
    pub error: HbError,
    pub delivery: Delivery,
}
impl From<HttpFailure> for HbError {
    fn from(value: HttpFailure) -> Self {
        value.error
    }
}

#[async_trait]
trait Resolver: Send + Sync {
    async fn resolve(&self, host: &str, port: u16) -> HbResult<Vec<SocketAddr>>;
}
struct SystemResolver;
#[async_trait]
impl Resolver for SystemResolver {
    async fn resolve(&self, host: &str, port: u16) -> HbResult<Vec<SocketAddr>> {
        let addresses = tokio::net::lookup_host((host, port))
            .await
            .map_err(|_| HbError::from(JsErrorKind::HttpFailed))?;
        Ok(addresses.take(MAX_ADDRESSES + 1).collect())
    }
}

// No fallback to the system resolver is possible inside reqwest, including after redirects.
struct PinnedDns {
    host: String,
    addresses: Vec<SocketAddr>,
}
impl reqwest::dns::Resolve for PinnedDns {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let result = if name.as_str() == self.host {
            Ok(Box::new(self.addresses.clone().into_iter()) as reqwest::dns::Addrs)
        } else {
            Err(Box::new(std::io::Error::other("unvalidated DNS name"))
                as Box<dyn std::error::Error + Send + Sync>)
        };
        Box::pin(async move { result })
    }
}

#[derive(Clone)]
pub struct HttpService {
    config: JsHttpConfig,
    origins: BTreeSet<String>,
    resolver: Arc<dyn Resolver>,
    // Explicit native test harnesses can use loopback receivers. No JS/config switch.
    #[cfg(any(test, feature = "test-support"))]
    loopback: bool,
    #[cfg(test)]
    roots: Vec<reqwest::Certificate>,
}

impl HttpService {
    pub fn new(config: JsHttpConfig) -> HbResult<Self> {
        let origins = config
            .allowlist
            .iter()
            .map(|origin| {
                exact_origin(origin)
                    .map_err(|_| HbError::validation("invalid HTTP origin allowlist"))
            })
            .collect::<HbResult<_>>()?;
        if config.max_request_bytes == 0
            || config.max_response_bytes == 0
            || config.connect_timeout_ms == 0
            || config.timeout_ms == 0
        {
            return Err(HbError::validation("HTTP limits must be positive"));
        }
        Ok(Self {
            config,
            origins,
            resolver: Arc::new(SystemResolver),
            #[cfg(any(test, feature = "test-support"))]
            loopback: false,
            #[cfg(test)]
            roots: Vec::new(),
        })
    }

    /// Native integration harness only; production construction retains public-IP checks.
    #[cfg(feature = "test-support")]
    pub fn for_loopback_test(config: JsHttpConfig) -> HbResult<Self> {
        let mut service = Self::new(config)?;
        service.loopback = true;
        Ok(service)
    }

    /// Used both at outbox enqueue and delivery. DNS is always checked again at delivery.
    pub async fn validate(&self, request: &HttpRequest) -> HbResult<()> {
        let (url, _, _) = self.prepare(request)?;
        tokio::time::timeout(
            Duration::from_millis(self.config.timeout_ms),
            self.addresses(&url),
        )
        .await
        .map_err(|_| HbError::from(JsErrorKind::HttpTimeout))??;
        Ok(())
    }

    pub async fn send(
        &self,
        request: HttpRequest,
        remaining_ms: u64,
    ) -> Result<HttpResponse, HttpFailure> {
        let prepared = self.prepare(&request).map_err(|error| HttpFailure {
            error,
            delivery: Delivery::NotSent,
        })?;
        let timeout = request
            .timeout_ms
            .unwrap_or(self.config.timeout_ms)
            .min(self.config.timeout_ms)
            .min(remaining_ms);
        if timeout == 0 {
            return Err(HttpFailure {
                error: JsErrorKind::HttpTimeout.into(),
                delivery: Delivery::NotSent,
            });
        }
        let mut delivery = Delivery::NotSent;
        let result = tokio::time::timeout(
            Duration::from_millis(timeout),
            self.send_inner(prepared, request.body, &mut delivery),
        )
        .await
        .unwrap_or_else(|_| Err(JsErrorKind::HttpTimeout.into()));
        result.map_err(|error| HttpFailure { error, delivery })
    }

    fn target(&self, url: &Url) -> HbResult<()> {
        if !matches!(url.scheme(), "http" | "https")
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
            || !self.origins.contains(&url.origin().ascii_serialization())
        {
            return Err(JsErrorKind::OutboundDenied.into());
        }
        let host = url
            .host_str()
            .ok_or_else(|| HbError::from(JsErrorKind::OutboundDenied))?;
        let host = host.trim_end_matches('.').to_ascii_lowercase();
        if host == "localhost"
            || host.ends_with(".localhost")
            || host.ends_with(".local")
            || host.ends_with(".internal")
            || host == "metadata.google.internal"
        {
            return Err(JsErrorKind::OutboundDenied.into());
        }
        Ok(())
    }

    fn prepare(&self, request: &HttpRequest) -> HbResult<(Url, Method, HeaderMap)> {
        if !self.config.enabled {
            return Err(JsErrorKind::Denied.into());
        }
        if has_url_userinfo(&request.url) {
            return Err(JsErrorKind::OutboundDenied.into());
        }
        if request.url.len() > MAX_URL_BYTES || request.headers.len() > MAX_HEADERS {
            return Err(HbError::PayloadTooLarge);
        }
        let url = Url::parse(&request.url).map_err(|_| HbError::validation("invalid HTTP URL"))?;
        self.target(&url)?;
        let method = Method::from_bytes(request.method.as_bytes())
            .map_err(|_| HbError::validation("invalid HTTP method"))?;
        if !matches!(
            method,
            Method::GET
                | Method::HEAD
                | Method::POST
                | Method::PUT
                | Method::PATCH
                | Method::DELETE
                | Method::OPTIONS
        ) || (matches!(method, Method::GET | Method::HEAD) && request.body.is_some())
            || request.timeout_ms == Some(0)
        {
            return Err(HbError::validation("invalid HTTP method, body or timeout"));
        }
        let mut headers = HeaderMap::new();
        let mut bytes = 0usize;
        for (name, value) in &request.headers {
            bytes = bytes.saturating_add(name.len()).saturating_add(value.len());
            if bytes > MAX_HEADER_BYTES {
                return Err(HbError::PayloadTooLarge);
            }
            let name = HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| HbError::validation("invalid HTTP header name"))?;
            if matches!(
                name.as_str(),
                "host"
                    | "connection"
                    | "content-length"
                    | "transfer-encoding"
                    | "te"
                    | "trailer"
                    | "upgrade"
                    | "expect"
                    | "proxy-authorization"
                    | "proxy-authenticate"
                    | "proxy-connection"
            ) {
                return Err(HbError::validation(
                    "HTTP transport header is managed by the host",
                ));
            }
            let value = HeaderValue::from_str(value)
                .map_err(|_| HbError::validation("invalid HTTP header value"))?;
            if headers.insert(name, value).is_some() {
                return Err(HbError::validation("duplicate HTTP header"));
            }
        }
        bytes = bytes
            .saturating_add(request.url.len())
            .saturating_add(request.method.len())
            .saturating_add(request.body.as_ref().map_or(0, String::len));
        if bytes > self.config.max_request_bytes {
            return Err(HbError::PayloadTooLarge);
        }
        // The response is a UTF-8 string; no implicit decompression is performed.
        headers.insert(
            reqwest::header::ACCEPT_ENCODING,
            HeaderValue::from_static("identity"),
        );
        Ok((url, method, headers))
    }

    async fn addresses(&self, url: &Url) -> HbResult<Vec<SocketAddr>> {
        self.target(url)?;
        let port = url
            .port_or_known_default()
            .ok_or_else(|| HbError::from(JsErrorKind::OutboundDenied))?;
        let addresses = match url
            .host()
            .ok_or_else(|| HbError::from(JsErrorKind::OutboundDenied))?
        {
            Host::Ipv4(ip) => vec![SocketAddr::new(ip.into(), port)],
            Host::Ipv6(ip) => vec![SocketAddr::new(ip.into(), port)],
            Host::Domain(host) => self.resolver.resolve(host, port).await?,
        };
        if addresses.is_empty()
            || addresses.len() > MAX_ADDRESSES
            || addresses.iter().any(|address| {
                #[cfg(any(test, feature = "test-support"))]
                if self.loopback && address.ip().is_loopback() {
                    return address.port() != port;
                }
                address.port() != port || !public_address(address.ip())
            })
        {
            return Err(JsErrorKind::OutboundDenied.into());
        }
        Ok(addresses)
    }

    fn client(&self, url: &Url, addresses: Vec<SocketAddr>) -> HbResult<Client> {
        let builder = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .referer(false)
            .http1_only()
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd()
            .connect_timeout(Duration::from_millis(self.config.connect_timeout_ms))
            .pool_max_idle_per_host(0)
            .dns_resolver(Arc::new(PinnedDns {
                host: url.host_str().unwrap_or_default().into(),
                addresses,
            }));
        #[cfg(test)]
        let builder = self.roots.iter().fold(builder, |builder, certificate| {
            builder.add_root_certificate(certificate.clone())
        });
        builder.build().map_err(|_| JsErrorKind::HttpFailed.into())
    }

    async fn send_inner(
        &self,
        (mut url, mut method, mut headers): (Url, Method, HeaderMap),
        mut body: Option<String>,
        delivery: &mut Delivery,
    ) -> HbResult<HttpResponse> {
        for hop in 0..=self.config.max_redirects {
            let addresses = self.addresses(&url).await?;
            let client = self.client(&url, addresses)?;
            let mut request = client
                .request(method.clone(), url.clone())
                .headers(headers.clone());
            if let Some(body) = &body {
                request = request.body(body.clone());
            }
            let previous = *delivery;
            *delivery = Delivery::Unknown;
            let response = request.send().await.map_err(|error| -> HbError {
                // Connection establishment failures occur before any request bytes.
                if error.is_connect() {
                    *delivery = previous;
                }
                if error.is_timeout() {
                    JsErrorKind::HttpTimeout.into()
                } else {
                    JsErrorKind::HttpFailed.into()
                }
            })?;
            let status = response.status();
            if response.headers().len() > MAX_HEADERS
                || response
                    .headers()
                    .iter()
                    .map(|(key, value)| key.as_str().len().saturating_add(value.as_bytes().len()))
                    .sum::<usize>()
                    > MAX_HEADER_BYTES
            {
                return Err(HbError::PayloadTooLarge);
            }
            if matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308)
                && let Some(location) = response.headers().get(LOCATION)
            {
                if hop == self.config.max_redirects {
                    return Err(JsErrorKind::OutboundDenied.into());
                }
                let location = location
                    .to_str()
                    .map_err(|_| HbError::from(JsErrorKind::OutboundDenied))?;
                if has_url_userinfo(location) {
                    return Err(JsErrorKind::OutboundDenied.into());
                }
                if location.len() > MAX_URL_BYTES {
                    return Err(HbError::PayloadTooLarge);
                }
                let next = url
                    .join(location)
                    .map_err(|_| HbError::from(JsErrorKind::OutboundDenied))?;
                self.target(&next)?;
                if url.scheme() == "https" && next.scheme() != "https" {
                    return Err(JsErrorKind::OutboundDenied.into());
                }
                if url.origin() != next.origin() {
                    // Custom API keys can use arbitrary names. Only representation headers survive.
                    headers = headers
                        .into_iter()
                        .filter_map(|(name, value)| {
                            name.filter(|name| {
                                matches!(
                                    name.as_str(),
                                    "accept"
                                        | "accept-language"
                                        | "accept-encoding"
                                        | "content-type"
                                )
                            })
                            .map(|name| (name, value))
                        })
                        .collect();
                }
                if (status.as_u16() == 303 && method != Method::HEAD)
                    || (matches!(status.as_u16(), 301 | 302) && method == Method::POST)
                {
                    method = Method::GET;
                    body = None;
                    headers.remove(reqwest::header::CONTENT_TYPE);
                    headers.remove(reqwest::header::CONTENT_ENCODING);
                }
                url = next;
                continue;
            }
            if response
                .content_length()
                .is_some_and(|size| size > self.config.max_response_bytes as u64)
            {
                return Err(HbError::PayloadTooLarge);
            }
            let headers: BTreeMap<_, _> = response
                .headers()
                .iter()
                .filter_map(|(key, value)| {
                    value
                        .to_str()
                        .ok()
                        .map(|value| (key.to_string(), value.to_owned()))
                })
                .collect();
            let mut bytes = Vec::new();
            let mut stream = response.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|_| HbError::from(JsErrorKind::HttpFailed))?;
                if chunk.len() > self.config.max_response_bytes.saturating_sub(bytes.len()) {
                    return Err(HbError::PayloadTooLarge);
                }
                bytes.extend_from_slice(&chunk);
            }
            let body =
                String::from_utf8(bytes).map_err(|_| HbError::from(JsErrorKind::HttpFailed))?;
            return Ok(HttpResponse {
                status: status.as_u16(),
                ok: status.is_success(),
                headers,
                body,
            });
        }
        Err(JsErrorKind::OutboundDenied.into())
    }
}

/// Conservative global-unicast policy, including embedded IPv4 and transition networks.
fn public_address(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(a == 0
                || a == 10
                || a == 127
                || a >= 224
                || (a == 100 && (64..=127).contains(&b))
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192 && (b == 168 || b == 0 || (b == 88 && c == 99)))
                || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
                || (a == 203 && b == 0 && c == 113))
        }
        IpAddr::V6(ip) => {
            let s = ip.segments();
            // Only ordinary 2000::/3 global unicast. Exclude Teredo, benchmarking,
            // ORCHID, documentation, 6to4 and all mapped/NAT64/local ranges.
            s[0] & 0xe000 == 0x2000
                && !(s[0] == 0x2001 && (s[1] < 0x0200 || s[1] == 0x0db8))
                && s[0] != 0x2002
                && !(s[0] == 0x3fff && s[1] < 0x1000)
        }
    }
}

#[cfg(test)]
mod tests;

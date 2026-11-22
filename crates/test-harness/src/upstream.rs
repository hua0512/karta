//! Fake upstream / fake CDN: one local server that answers for every host.
//!
//! The intercepting client ([`FakeUpstream::client`]) is the production
//! client builder plus a DNS resolver that maps *every* hostname to this
//! server, with certificate verification off. Parsers keep their real
//! hard-coded `https://` URLs, and redirects wreq follows internally land
//! here too, so requests never leave the machine.
//!
//! The listener sniffs the first byte of each connection: `0x16` starts a
//! TLS handshake (self-signed certificate generated at startup, ALPN pinned
//! to `http/1.1`), anything else is served as plain HTTP/1.1. Requests are
//! routed on `Host` + method + path (+ selected query pairs).
//!
//! Requests the client sends to an IP literal or an explicit `:port` bypass
//! the resolver; parsers do not do either.
//!
//! Do not combine with `#[tokio::test(start_paused = true)]`: paused time
//! auto-advances while the client waits on socket I/O, so client timeouts
//! fire spuriously.
//!
//! ```ignore
//! let fake = FakeUpstream::start().await;
//! let page = fake
//!     .route("www.example.com", "GET", "/post?id=1")
//!     .reply(Reply::ok().json(&serde_json::json!({ "ok": true })));
//! let client = fake.client(&Config::default());
//! client.get("https://www.example.com/post?id=1&x=2").send().await?;
//! assert_eq!(page.hits(), 1);
//! fake.assert_no_unmatched();
//! ```

use std::io::Write as _;
use std::net::SocketAddr;
use std::path::Path;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use btls::ssl::{AlpnError, NameType, Ssl, SslAcceptor, SslMethod};
use karta_core::Config;
use karta_core::download::{SharedDownloader, StreamDownloader};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Notify, watch};
use wreq::dns::{Addrs, Name, Resolve, Resolving};

/// Status returned for requests that match no route.
pub const UNMATCHED_STATUS: u16 = 599;

const MAX_HEADERS: usize = 128;

/// A local server impersonating every upstream host.
///
/// Dropping it stops the listener and releases every open connection,
/// including ones parked by [`Reply::hang`].
pub struct FakeUpstream {
    addr: SocketAddr,
    inner: Arc<Inner>,
    shutdown: watch::Sender<bool>,
}

#[derive(Default)]
struct Inner {
    routes: Mutex<Vec<Route>>,
    requests: Mutex<Vec<CapturedRequest>>,
    unmatched: Mutex<Vec<CapturedRequest>>,
    activity: Notify,
}

impl FakeUpstream {
    /// Bind `127.0.0.1:0` and start serving.
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind fake upstream");
        let addr = listener.local_addr().expect("fake upstream addr");
        let acceptor = Arc::new(tls_acceptor());
        let inner = Arc::new(Inner::default());
        let (shutdown, shutdown_rx) = watch::channel(false);

        let server_inner = Arc::clone(&inner);
        tokio::spawn(async move {
            let mut stop = shutdown_rx.clone();
            loop {
                let (tcp, _) = tokio::select! {
                    accepted = listener.accept() => match accepted {
                        Ok(conn) => conn,
                        Err(_) => continue,
                    },
                    _ = stop.changed() => return,
                };
                let inner = Arc::clone(&server_inner);
                let acceptor = Arc::clone(&acceptor);
                let mut stop = shutdown_rx.clone();
                tokio::spawn(async move {
                    tokio::select! {
                        _ = accept_connection(tcp, inner, acceptor) => {}
                        _ = stop.changed() => {}
                    }
                });
            }
        });

        Self {
            addr,
            inner,
            shutdown,
        }
    }

    /// The listener address.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Plain-HTTP base URL (`http://127.0.0.1:PORT`) for configs that take
    /// a URL rather than going through the resolver (e.g. `emoji_cdn_url`).
    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Register a route. `path` may carry a query string; each `key=value`
    /// pair (or bare `key`) in it must then be present in the request.
    ///
    /// Routes are matched in registration order; the first match wins.
    pub fn route(&self, host: &str, method: &str, path: &str) -> Route {
        self.add_route(host, method, path, false)
    }

    /// Like [`Self::route`], but the request path only has to start with
    /// `path`, for paths that embed a value the test cannot predict.
    pub fn route_prefix(&self, host: &str, method: &str, path: &str) -> Route {
        self.add_route(host, method, path, true)
    }

    fn add_route(&self, host: &str, method: &str, path: &str, path_prefix: bool) -> Route {
        let (path, query) = match path.split_once('?') {
            Some((path, query)) => (path, parse_query(query)),
            None => (path, Vec::new()),
        };
        let route = Route {
            state: Arc::new(RouteState {
                host: host.to_ascii_lowercase(),
                method: method.to_ascii_uppercase(),
                path: path.to_string(),
                path_prefix,
                query,
                script: Mutex::new(Vec::new()),
                requests: Mutex::new(Vec::new()),
            }),
        };
        self.inner.routes.lock().unwrap().push(route.clone());
        route
    }

    /// Every request received so far, matched or not, in arrival order.
    pub fn requests(&self) -> Vec<CapturedRequest> {
        self.inner.requests.lock().unwrap().clone()
    }

    /// Requests that matched no route.
    pub fn unmatched(&self) -> Vec<CapturedRequest> {
        self.inner.unmatched.lock().unwrap().clone()
    }

    /// Panic listing every request that matched no route.
    #[track_caller]
    pub fn assert_no_unmatched(&self) {
        let unmatched = self.unmatched();
        assert!(
            unmatched.is_empty(),
            "fake upstream received unrouted requests:\n{}",
            unmatched
                .iter()
                .map(|r| format!("  {} {}{}", r.method, r.host, r.uri()))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    /// Wait until `route` has been hit at least `n` times.
    ///
    /// Panics after `timeout`; the timeout only bounds hangs.
    pub async fn wait_for_hits(&self, route: &Route, n: usize, timeout: Duration) {
        let wait = async {
            loop {
                let notified = self.inner.activity.notified();
                if route.hits() >= n {
                    return;
                }
                notified.await;
            }
        };
        if tokio::time::timeout(timeout, wait).await.is_err() {
            panic!(
                "route {} {}{} saw {} hits, expected {n} within {timeout:?}",
                route.state.method,
                route.state.host,
                route.state.path,
                route.hits()
            );
        }
    }

    /// Add interception to a client builder: every hostname resolves to
    /// this server, certificates are not verified, system proxies are off.
    pub fn intercept(&self, builder: wreq::ClientBuilder) -> wreq::ClientBuilder {
        builder
            .dns_resolver(LoopbackResolver(self.addr))
            .tls_cert_verification(false)
            .no_proxy()
    }

    /// The production client (emulation, redirects, timeouts) with
    /// interception added.
    pub fn client(&self, config: &Config) -> wreq::Client {
        let builder =
            StreamDownloader::production_client_builder(config).expect("production client config");
        self.intercept(builder)
            .build()
            .expect("build intercepting client")
    }

    /// A downloader whose client is [`Self::client`].
    pub fn downloader(&self, config: &Config, data_path: &Path) -> SharedDownloader {
        Arc::new(
            StreamDownloader::with_client(config, data_path, self.client(config))
                .expect("build downloader"),
        )
    }
}

impl Drop for FakeUpstream {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
    }
}

/// Resolves every name to the fake upstream's address. A non-zero port from
/// a custom resolver is kept by wreq when the URI has no explicit port.
struct LoopbackResolver(SocketAddr);

impl Resolve for LoopbackResolver {
    fn resolve(&self, _name: Name) -> Resolving {
        let addr = self.0;
        Box::pin(async move { Ok(Box::new(std::iter::once(addr)) as Addrs) })
    }
}

/// A registered route: its reply script and the requests it received.
#[derive(Clone)]
pub struct Route {
    state: Arc<RouteState>,
}

struct RouteState {
    host: String,
    method: String,
    path: String,
    /// `path` is a prefix rather than the whole path.
    path_prefix: bool,
    query: Vec<(String, Option<String>)>,
    script: Mutex<Vec<Reply>>,
    requests: Mutex<Vec<CapturedRequest>>,
}

impl Route {
    /// Append a reply to the script: the n-th hit gets the n-th reply, and
    /// the last reply repeats once the script is exhausted. A route without
    /// replies answers `200` with an empty body.
    pub fn reply(self, reply: Reply) -> Self {
        self.state.script.lock().unwrap().push(reply);
        self
    }

    /// How many requests this route answered.
    pub fn hits(&self) -> usize {
        self.state.requests.lock().unwrap().len()
    }

    /// Requests this route answered, in arrival order.
    pub fn requests(&self) -> Vec<CapturedRequest> {
        self.state.requests.lock().unwrap().clone()
    }

    /// The most recent request; panics if the route was never hit.
    #[track_caller]
    pub fn last_request(&self) -> CapturedRequest {
        self.state
            .requests
            .lock()
            .unwrap()
            .last()
            .cloned()
            .unwrap_or_else(|| {
                panic!(
                    "route {} {}{} was never hit",
                    self.state.method, self.state.host, self.state.path
                )
            })
    }

    fn matches(&self, req: &CapturedRequest) -> bool {
        let state = &self.state;
        state.host == req.host
            && state.method == req.method
            && if state.path_prefix {
                req.path.starts_with(&state.path)
            } else {
                state.path == req.path
            }
            && state.query.iter().all(|(key, value)| {
                req.query_pairs()
                    .iter()
                    .any(|(k, v)| k == key && value.as_ref().is_none_or(|value| v == value))
            })
    }
}

/// A request as the fake upstream received it.
#[derive(Clone, Debug)]
pub struct CapturedRequest {
    /// Upper-case method.
    pub method: String,
    /// Lower-case `Host` header without port.
    pub host: String,
    /// Path without query.
    pub path: String,
    /// Raw query string, without `?`.
    pub query: Option<String>,
    /// Headers in arrival order, names lower-cased.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// Whether the request arrived over TLS.
    pub tls: bool,
    /// TLS server name indication, when sent.
    pub sni: Option<String>,
}

impl CapturedRequest {
    /// Path plus `?query`.
    pub fn uri(&self) -> String {
        match &self.query {
            Some(query) => format!("{}?{query}", self.path),
            None => self.path.clone(),
        }
    }

    /// First header named `name` (case-insensitive).
    pub fn header(&self, name: &str) -> Option<&str> {
        let name = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| v.as_str())
    }

    /// Query pairs, not percent-decoded.
    pub fn query_pairs(&self) -> Vec<(String, String)> {
        parse_query(self.query.as_deref().unwrap_or(""))
            .into_iter()
            .map(|(k, v)| (k, v.unwrap_or_default()))
            .collect()
    }

    /// First value of query key `key`, not percent-decoded.
    pub fn query_param(&self, key: &str) -> Option<String> {
        self.query_pairs()
            .into_iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }

    /// Body as UTF-8 (lossy).
    pub fn body_str(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

fn parse_query(query: &str) -> Vec<(String, Option<String>)> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| match pair.split_once('=') {
            Some((k, v)) => (k.to_string(), Some(v.to_string())),
            None => (pair.to_string(), None),
        })
        .collect()
}

/// How a reply delimits its body.
#[derive(Clone, Debug)]
enum Framing {
    ContentLength,
    Chunked,
    /// No `Content-Length`; the body ends when the connection closes.
    CloseDelimited,
    /// Announces `declared` bytes, sends the body, then closes.
    Truncated {
        declared: usize,
    },
}

/// A scripted response.
#[derive(Clone, Debug)]
pub struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    gzip: bool,
    framing: Framing,
    hang: bool,
}

impl Reply {
    /// A reply with `status` and an empty body.
    pub fn status(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
            gzip: false,
            framing: Framing::ContentLength,
            hang: false,
        }
    }

    /// `200` with an empty body.
    pub fn ok() -> Self {
        Self::status(200)
    }

    /// `302` to `location` (absolute URLs may point at any host).
    pub fn redirect(location: &str) -> Self {
        Self::status(302).header("location", location)
    }

    /// Never answer; the connection stays open until the client gives up or
    /// the server is dropped.
    pub fn hang() -> Self {
        Self {
            hang: true,
            ..Self::ok()
        }
    }

    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }

    pub fn content_type(self, value: &str) -> Self {
        self.header("content-type", value)
    }

    pub fn set_cookie(self, cookie: &str) -> Self {
        self.header("set-cookie", cookie)
    }

    pub fn body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = body.into();
        self
    }

    /// JSON body with `content-type: application/json`.
    pub fn json(self, value: &serde_json::Value) -> Self {
        self.content_type("application/json")
            .body(serde_json::to_vec(value).expect("serialize json reply"))
    }

    /// Body read from a file; panics if it cannot be read.
    #[track_caller]
    pub fn fixture(self, path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        let body =
            std::fs::read(path).unwrap_or_else(|e| panic!("read fixture {}: {e}", path.display()));
        self.body(body)
    }

    /// Gzip the body and send `content-encoding: gzip`.
    pub fn gzip(mut self) -> Self {
        self.gzip = true;
        self
    }

    /// `transfer-encoding: chunked` instead of `content-length`.
    pub fn chunked(mut self) -> Self {
        self.framing = Framing::Chunked;
        self
    }

    /// No `content-length`; close the connection after the body.
    pub fn no_content_length(mut self) -> Self {
        self.framing = Framing::CloseDelimited;
        self
    }

    /// Announce `declared` bytes in `content-length`, send the (shorter)
    /// body, then close the connection.
    pub fn truncated(mut self, declared: usize) -> Self {
        self.framing = Framing::Truncated { declared };
        self
    }
}

fn tls_acceptor() -> SslAcceptor {
    use btls::asn1::Asn1Time;
    use btls::ec::{EcGroup, EcKey};
    use btls::hash::MessageDigest;
    use btls::nid::Nid;
    use btls::pkey::PKey;
    use btls::x509::{X509, X509NameBuilder};

    let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).expect("p-256 group");
    let key = PKey::from_ec_key(EcKey::generate(&group).expect("ec key")).expect("pkey");

    let mut name = X509NameBuilder::new().expect("x509 name");
    name.append_entry_by_text("CN", "fake-upstream")
        .expect("x509 cn");
    let name = name.build();

    let mut cert = X509::builder().expect("x509 builder");
    cert.set_version(2).expect("x509 version");
    cert.set_subject_name(&name).expect("x509 subject");
    cert.set_issuer_name(&name).expect("x509 issuer");
    cert.set_pubkey(&key).expect("x509 pubkey");
    cert.set_not_before(&Asn1Time::days_from_now(0).expect("asn1 time"))
        .expect("x509 not_before");
    cert.set_not_after(&Asn1Time::days_from_now(1).expect("asn1 time"))
        .expect("x509 not_after");
    cert.sign(&key, MessageDigest::sha256()).expect("x509 sign");
    let cert = cert.build();

    let mut acceptor =
        SslAcceptor::mozilla_intermediate_v5(SslMethod::tls()).expect("tls acceptor");
    acceptor.set_private_key(&key).expect("tls key");
    acceptor.set_certificate(&cert).expect("tls cert");
    // Clients offering h2 (browser emulation does) are steered to HTTP/1.1,
    // the only protocol this server speaks.
    acceptor.set_alpn_select_callback(|_, client| {
        btls::ssl::select_next_proto(b"\x08http/1.1", client).ok_or(AlpnError::NOACK)
    });
    acceptor.build()
}

async fn accept_connection(tcp: TcpStream, inner: Arc<Inner>, acceptor: Arc<SslAcceptor>) {
    let mut first = [0u8; 1];
    match tcp.peek(&mut first).await {
        Ok(1) => {}
        _ => return,
    }
    if first[0] != 0x16 {
        serve(tcp, &inner, false, None).await;
        return;
    }

    let Ok(ssl) = Ssl::new(acceptor.context()) else {
        return;
    };
    let Ok(mut tls) = tokio_btls::SslStream::new(ssl, tcp) else {
        return;
    };
    if Pin::new(&mut tls).accept().await.is_err() {
        return;
    }
    let sni = tls
        .ssl()
        .servername(NameType::HOST_NAME)
        .map(str::to_string);
    serve(tls, &inner, true, sni).await;
}

/// Serve HTTP/1.1 requests on one connection until it closes.
async fn serve<S>(mut io: S, inner: &Inner, tls: bool, sni: Option<String>)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut buf = Vec::with_capacity(8 * 1024);
    loop {
        let Some(mut req) = read_request(&mut io, &mut buf).await else {
            return;
        };
        req.tls = tls;
        req.sni = sni.clone();
        let close_requested = req
            .header("connection")
            .is_some_and(|v| v.eq_ignore_ascii_case("close"));
        let head_only = req.method == "HEAD";

        let reply = dispatch(inner, req);
        if reply.hang {
            std::future::pending::<()>().await;
        }
        let keep_alive = write_reply(&mut io, &reply, head_only)
            .await
            .unwrap_or(false);
        if !keep_alive || close_requested {
            let _ = io.shutdown().await;
            return;
        }
    }
}

fn dispatch(inner: &Inner, req: CapturedRequest) -> Reply {
    inner.requests.lock().unwrap().push(req.clone());
    let route = inner
        .routes
        .lock()
        .unwrap()
        .iter()
        .find(|route| route.matches(&req))
        .cloned();

    let reply = match route {
        Some(route) => {
            let hit = {
                let mut requests = route.state.requests.lock().unwrap();
                requests.push(req);
                requests.len()
            };
            let script = route.state.script.lock().unwrap();
            script
                .get(hit - 1)
                .or_else(|| script.last())
                .cloned()
                .unwrap_or_else(Reply::ok)
        }
        None => {
            let body = format!(
                "fake upstream: no route for {} {}{}",
                req.method,
                req.host,
                req.uri()
            );
            inner.unmatched.lock().unwrap().push(req);
            Reply::status(UNMATCHED_STATUS).body(body)
        }
    };
    inner.activity.notify_waiters();
    reply
}

/// Read one request (head and body). `None` when the connection ends or the
/// request is malformed.
async fn read_request<S>(io: &mut S, buf: &mut Vec<u8>) -> Option<CapturedRequest>
where
    S: AsyncRead + Unpin,
{
    let (head_len, mut req, content_length, chunked) = loop {
        let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
        let mut parsed = httparse::Request::new(&mut headers);
        match parsed.parse(buf).ok()? {
            httparse::Status::Complete(head_len) => {
                let headers: Vec<(String, String)> = parsed
                    .headers
                    .iter()
                    .map(|h| {
                        (
                            h.name.to_ascii_lowercase(),
                            String::from_utf8_lossy(h.value).into_owned(),
                        )
                    })
                    .collect();
                let target = parsed.path?.to_string();
                let (path, query) = match target.split_once('?') {
                    Some((path, query)) => (path.to_string(), Some(query.to_string())),
                    None => (target, None),
                };
                let mut req = CapturedRequest {
                    method: parsed.method?.to_ascii_uppercase(),
                    host: String::new(),
                    path,
                    query,
                    headers,
                    body: Vec::new(),
                    tls: false,
                    sni: None,
                };
                req.host = req
                    .header("host")
                    .map(|h| strip_port(h).to_ascii_lowercase())
                    .unwrap_or_default();
                let content_length = req
                    .header("content-length")
                    .and_then(|v| v.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                let chunked = req
                    .header("transfer-encoding")
                    .is_some_and(|v| v.to_ascii_lowercase().contains("chunked"));
                break (head_len, req, content_length, chunked);
            }
            httparse::Status::Partial => {
                if !fill(io, buf).await {
                    return None;
                }
            }
        }
    };
    buf.drain(..head_len);

    if chunked {
        req.body = read_chunked(io, buf).await?;
    } else {
        while buf.len() < content_length {
            if !fill(io, buf).await {
                return None;
            }
        }
        req.body = buf.drain(..content_length).collect();
    }
    Some(req)
}

async fn read_chunked<S>(io: &mut S, buf: &mut Vec<u8>) -> Option<Vec<u8>>
where
    S: AsyncRead + Unpin,
{
    let mut body = Vec::new();
    loop {
        let line_end = loop {
            if let Some(pos) = buf.windows(2).position(|w| w == b"\r\n") {
                break pos;
            }
            if !fill(io, buf).await {
                return None;
            }
        };
        let size_line = String::from_utf8_lossy(&buf[..line_end]).into_owned();
        let size_hex = size_line.split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(size_hex, 16).ok()?;
        buf.drain(..line_end + 2);
        if size == 0 {
            // Trailers are not used by clients here; consume up to the blank line.
            loop {
                if let Some(pos) = buf.windows(2).position(|w| w == b"\r\n") {
                    let empty = pos == 0;
                    buf.drain(..pos + 2);
                    if empty {
                        return Some(body);
                    }
                } else if !fill(io, buf).await {
                    return None;
                }
            }
        }
        while buf.len() < size + 2 {
            if !fill(io, buf).await {
                return None;
            }
        }
        body.extend(buf.drain(..size));
        buf.drain(..2);
    }
}

async fn fill<S>(io: &mut S, buf: &mut Vec<u8>) -> bool
where
    S: AsyncRead + Unpin,
{
    let mut chunk = [0u8; 8 * 1024];
    match io.read(&mut chunk).await {
        Ok(0) | Err(_) => false,
        Ok(n) => {
            buf.extend_from_slice(&chunk[..n]);
            true
        }
    }
}

fn strip_port(host: &str) -> &str {
    if host.starts_with('[') {
        return host.split_once(']').map_or(host, |(h, _)| &h[1..]);
    }
    host.rsplit_once(':').map_or(host, |(h, _)| h)
}

/// Write `reply`; returns whether the connection may be reused.
async fn write_reply<S>(io: &mut S, reply: &Reply, head_only: bool) -> std::io::Result<bool>
where
    S: AsyncWrite + Unpin,
{
    let body = if reply.gzip {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&reply.body)?;
        encoder.finish()?
    } else {
        reply.body.clone()
    };

    let reason = wreq::StatusCode::from_u16(reply.status)
        .ok()
        .and_then(|s| s.canonical_reason())
        .unwrap_or("Fake");
    let mut head = format!("HTTP/1.1 {} {reason}\r\n", reply.status);
    for (name, value) in &reply.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    if reply.gzip {
        head.push_str("content-encoding: gzip\r\n");
    }
    let keep_alive = match reply.framing {
        Framing::ContentLength => {
            head.push_str(&format!("content-length: {}\r\n", body.len()));
            true
        }
        Framing::Chunked => {
            head.push_str("transfer-encoding: chunked\r\n");
            true
        }
        Framing::CloseDelimited => {
            head.push_str("connection: close\r\n");
            false
        }
        Framing::Truncated { declared } => {
            head.push_str(&format!(
                "content-length: {declared}\r\nconnection: close\r\n"
            ));
            false
        }
    };
    head.push_str("\r\n");
    io.write_all(head.as_bytes()).await?;

    if !head_only {
        match reply.framing {
            Framing::Chunked => {
                if !body.is_empty() {
                    io.write_all(format!("{:x}\r\n", body.len()).as_bytes())
                        .await?;
                    io.write_all(&body).await?;
                    io.write_all(b"\r\n").await?;
                }
                io.write_all(b"0\r\n\r\n").await?;
            }
            _ => io.write_all(&body).await?,
        }
    }
    io.flush().await?;
    Ok(keep_alive)
}

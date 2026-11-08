//! Safe loopback HTTP transport. Workers own sockets and queue requests; all
//! backend calls (including accessor reads) run on the simulation thread.
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use xphost_core::{Backend, Request};

#[derive(Debug, Clone)]
pub struct Response {
    pub code: u16,
    pub body: Value,
}
impl Response {
    fn ok(data: Value) -> Self {
        Self {
            code: 200,
            body: json!({"data":data}),
        }
    }
    fn error(code: u16, text: &str) -> Self {
        Self {
            code,
            body: json!({"error":text}),
        }
    }
}
#[derive(Default)]
pub struct Router {
    ends: Vec<(String, f64)>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Patch {
    data: Value,
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Activation {
    #[serde(default)]
    duration: f64,
}
fn decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let hex = |b: u8| match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    };
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(a), Some(b)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(a * 16 + b);
                i += 3;
                continue;
            }
        }
        out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
fn parse_id(text: &str) -> Option<i32> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        None
    } else {
        text.parse().ok()
    }
}
impl Router {
    pub fn tick(&mut self, api: &impl Backend) {
        let now = api.time();
        self.ends.retain(|(name, at)| {
            if *at <= now {
                let _ = api.command(name, 2);
                false
            } else {
                true
            }
        });
    }
    pub fn finish(&mut self, api: &impl Backend) {
        for (name, _) in self.ends.drain(..) {
            let _ = api.command(&name, 2);
        }
    }
    pub fn route(
        &mut self,
        api: &impl Backend,
        method: &str,
        target: &str,
        body: &str,
    ) -> Response {
        let (path, query) = target.split_once('?').unwrap_or((target, ""));
        if path == "/" || path.is_empty() {
            return Response::ok(json!("xphost Web API v3 subset"));
        }
        let seg: Vec<_> = path
            .trim_start_matches('/')
            .split('/')
            .map(decode)
            .collect();
        if seg.len() < 3 || seg[0] != "api" || seg[1] != "v3" {
            return Response::error(404, "not found");
        }
        let filter = query
            .split('&')
            .filter_map(|p| p.split_once('='))
            .find_map(|(k, v)| {
                if matches!(decode(k).as_str(), "filter[name]" | "name") {
                    Some(decode(v))
                } else {
                    None
                }
            })
            .unwrap_or_default();
        let kind = seg[2].as_str();
        let refs = kind == "datarefs";
        let commands = matches!(kind, "command" | "commands");
        if !refs && !commands {
            return Response::error(404, "not found");
        }
        if seg.len() == 3 && method == "GET" {
            let list = match api.request(if refs {
                Request::Refs
            } else {
                Request::Commands
            }) {
                Ok(Value::Array(list)) => list,
                Ok(_) => return Response::error(500, "invalid backend metadata"),
                Err(e) => return Response::error(500, &e),
            };
            return Response::ok(json!(
                list.into_iter()
                    .filter(|v| filter.is_empty() || v["name"] == filter)
                    .collect::<Vec<_>>()
            ));
        }
        if seg.len() < 4 {
            return Response::error(404, "not found");
        }
        let Some(id) = parse_id(&seg[3]) else {
            return Response::error(400, "invalid id");
        };
        let metadata = match api.request(if refs {
            Request::Ref(id as usize)
        } else {
            Request::CommandInfo(id as usize)
        }) {
            Ok(Value::Null) => return Response::error(404, "no such id"),
            Ok(metadata) => metadata,
            Err(e) => return Response::error(500, &e),
        };
        let Some(name) = metadata["name"].as_str() else {
            return Response::error(500, "invalid metadata name");
        };
        if refs && seg.len() == 4 && method == "GET" {
            return Response::ok(metadata.clone());
        }
        if refs && seg.len() == 5 && seg[4] == "value" {
            if method == "PATCH" || method == "POST" {
                if !metadata["is_writable"].as_bool().unwrap_or(false) {
                    return Response::error(400, "read-only dataref");
                }
                let Ok(value) = serde_json::from_str::<Patch>(body) else {
                    return Response::error(400, "expected {\"data\": value}");
                };
                if let Err(e) = api.write(name, value.data, 0) {
                    return Response::error(400, &e);
                }
            } else if method != "GET" {
                return Response::error(404, "not found");
            }
            return match api.read(name) {
                Ok(v) => Response::ok(v),
                Err(e) => Response::error(400, &e),
            };
        }
        if commands && seg.len() == 5 && seg[4] == "activate" && matches!(method, "POST" | "PATCH")
        {
            let activation = if body.trim().is_empty() {
                Ok(Activation::default())
            } else {
                serde_json::from_str::<Activation>(body)
            };
            let Ok(Activation { duration }) = activation else {
                return Response::error(400, "expected a finite nonnegative duration");
            };
            if !duration.is_finite() || duration < 0. {
                return Response::error(400, "expected a finite nonnegative duration");
            }
            if let Err(e) = api.command(name, if duration > 0. { 1 } else { 0 }) {
                return Response::error(400, &e);
            }
            if duration > 0. {
                self.ends.push((name.into(), api.time() + duration));
            }
            return Response::ok(json!({}));
        }
        Response::error(404, "not found")
    }
}

struct Work {
    method: String,
    target: String,
    body: String,
    status: Arc<AtomicU8>,
    response: mpsc::Sender<Response>,
}
fn queued(
    tx: &mpsc::Sender<Work>,
    method: String,
    target: String,
    body: String,
    timeout: Duration,
) -> Response {
    let status = Arc::new(AtomicU8::new(0));
    let (response, rx) = mpsc::channel();
    if tx
        .send(Work {
            method,
            target,
            body,
            status: status.clone(),
            response,
        })
        .is_err()
    {
        return Response::error(503, "host is stopping");
    }
    match rx.recv_timeout(timeout) {
        Ok(value) => value,
        Err(_) => {
            // The same CAS arbitrates execution versus timeout. Once cancelled,
            // queued writes/commands cannot later begin on the simulator thread.
            let _ = status.compare_exchange(0, 2, Ordering::AcqRel, Ordering::Acquire);
            Response::error(504, "sim thread timed out")
        }
    }
}
fn drain(rx: &mpsc::Receiver<Work>, router: &mut Router, api: &impl Backend) {
    // Bound each batch so a busy HTTP client cannot starve flight loops.
    for work in rx.try_iter().take(128) {
        if work
            .status
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            let result = router.route(api, &work.method, &work.target, &work.body);
            let _ = work.response.send(result);
        }
    }
}
const MAX_REQUEST: usize = 262144;
fn fill(
    stream: &mut TcpStream,
    buffer: &mut Vec<u8>,
    need: usize,
    stop: &AtomicBool,
) -> io::Result<bool> {
    if need > MAX_REQUEST {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "request too large",
        ));
    }
    let mut chunk = [0u8; 16384];
    while buffer.len() < need {
        if stop.load(Ordering::Relaxed) {
            return Ok(false);
        }
        match stream.read(&mut chunk) {
            Ok(0) => return Ok(false),
            Ok(n) => {
                buffer.extend_from_slice(&chunk[..n]);
                if buffer.len() > MAX_REQUEST {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "request too large",
                    ));
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                continue;
            }
            Err(e) => return Err(e),
        }
    }
    Ok(true)
}
fn connection(
    mut stream: TcpStream,
    tx: mpsc::Sender<Work>,
    stop: Arc<AtomicBool>,
) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_millis(250)))?;
    stream.set_write_timeout(Some(Duration::from_millis(250)))?;
    stream.set_nodelay(true)?;
    let mut buffer = Vec::new();
    loop {
        let head_end = loop {
            if let Some(i) = buffer.windows(4).position(|s| s == b"\r\n\r\n") {
                break i;
            }
            let need = buffer.len() + 1;
            if !fill(&mut stream, &mut buffer, need, &stop)? {
                return Ok(());
            }
        };
        let head = std::str::from_utf8(&buffer[..head_end])
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid header"))?;
        let mut lines = head.split("\r\n");
        let first = lines.next().unwrap_or("");
        let words: Vec<_> = first.split_whitespace().collect();
        if words.len() != 3 {
            return Ok(());
        }
        let method = words[0].to_owned();
        let target = words[1].to_owned();
        let mut length = None;
        let mut close = false;
        for line in lines {
            let Some((key, value)) = line.split_once(':') else {
                return Ok(());
            };
            let value = value.trim();
            if key.eq_ignore_ascii_case("content-length") {
                if length.is_some() {
                    return Ok(());
                }
                length =
                    Some(value.parse::<usize>().map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidData, "invalid length")
                    })?);
            }
            if key.eq_ignore_ascii_case("transfer-encoding") {
                return Ok(());
            }
            if key.eq_ignore_ascii_case("connection") {
                close = value.eq_ignore_ascii_case("close");
            }
        }
        let length = length.unwrap_or(0);
        let Some(end) = (head_end + 4).checked_add(length) else {
            return Ok(());
        };
        if !fill(&mut stream, &mut buffer, end, &stop)? {
            return Ok(());
        }
        let body = String::from_utf8_lossy(&buffer[head_end + 4..end]).into_owned();
        buffer.drain(..end);
        let stopping = method == "POST" && target == "/xphost/stop";
        let result = if stopping {
            Response::ok(json!({}))
        } else {
            queued(&tx, method, target, body, Duration::from_secs(5))
        };
        let body = result.body.to_string();
        let header = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: {}\r\n\r\n",
            result.code,
            if result.code == 200 { "OK" } else { "Error" },
            body.len(),
            if close || stopping {
                "close"
            } else {
                "keep-alive"
            }
        );
        stream.write_all(header.as_bytes())?;
        stream.write_all(body.as_bytes())?;
        if stopping {
            stop.store(true, Ordering::Release);
            println!("xphost serve: stop requested");
            return Ok(());
        }
        if close {
            return Ok(());
        }
    }
}
pub fn run(
    api: &impl Backend,
    port: u16,
    dt: f64,
    fast: bool,
    stop: Arc<AtomicBool>,
) -> Result<(), String> {
    let listener = TcpListener::bind(("127.0.0.1", port))
        .map_err(|e| format!("cannot listen on 127.0.0.1:{port}: {e}"))?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    println!(
        "xphost serve: http://127.0.0.1:{port}/api/v3 (clock {}, dt {dt:.4}); Ctrl-C stops",
        if fast { "fast" } else { "real" }
    );
    let (tx, rx) = mpsc::channel();
    let mut router = Router::default();
    let mut workers: Vec<thread::JoinHandle<()>> = Vec::new();
    let mut next = Instant::now();
    let pace = Duration::from_secs_f64(if fast { 0.0005 } else { dt });
    let result = (|| {
        while !stop.load(Ordering::Acquire) {
            for _ in 0..32 {
                let accepted = listener.accept();
                match accepted {
                    Ok((stream, _)) => {
                        if workers.len() < 128 {
                            let tx = tx.clone();
                            let stop = stop.clone();
                            workers.push(thread::spawn(move || {
                                let _ = connection(stream, tx, stop);
                            }));
                        }
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                    Err(e) => return Err(e.to_string()),
                }
            }
            let mut i = 0;
            while i < workers.len() {
                if workers[i].is_finished() {
                    let _ = workers.swap_remove(i).join();
                } else {
                    i += 1;
                }
            }
            drain(&rx, &mut router, api);
            api.step(dt)?;
            router.tick(api);
            next += pace;
            let now = Instant::now();
            if next > now {
                thread::sleep(next - now);
            } else if now.duration_since(next) > Duration::from_millis(250) {
                next = now;
            }
        }
        Ok(())
    })();
    stop.store(true, Ordering::Release);
    drop(listener);
    drop(rx);
    drop(tx);
    router.finish(api);
    for worker in workers {
        let _ = worker.join();
    }
    result
}

pub fn selftest(api: &impl Backend, dt: f64) -> usize {
    let mut router = Router::default();
    let mut failures = 0;
    let mut check = |name: &str, ok: bool| {
        println!("selftest {name:<38} {}", if ok { "PASS" } else { "FAIL" });
        if !ok {
            failures += 1;
        }
    };
    let refs = router.route(api, "GET", "/api/v3/datarefs", "");
    check("dataref enumeration", refs.code == 200);
    let id = |name: &str| {
        refs.body["data"]
            .as_array()
            .and_then(|a| a.iter().find(|v| v["name"] == name))
            .and_then(|v| v["id"].as_u64())
            .unwrap_or(u64::MAX)
    };
    let path = |name: &str| format!("/api/v3/datarefs/{}/value", id(name));
    check(
        "fixture ready",
        api.read("xphost/test/ready").ok().and_then(|v| v.as_f64()) == Some(1.),
    );
    check(
        "owned scalar PATCH",
        router
            .route(api, "PATCH", &path("xphost/test/value"), r#"{"data":3.75}"#)
            .code
            == 200
            && api.read("xphost/test/value").ok() == Some(json!(3.75)),
    );
    check(
        "owned byte PATCH/GET base64",
        router
            .route(
                api,
                "PATCH",
                &path("xphost/test/bytes"),
                r#"{"data":"aGVsbG8="}"#,
            )
            .body["data"]
            == "aGVsbG8=",
    );
    check(
        "owned array PATCH/GET",
        router
            .route(
                api,
                "PATCH",
                &path("xphost/test/array"),
                r#"{"data":[4,3,2,1]}"#,
            )
            .body["data"]
            == json!([4., 3., 2., 1.]),
    );
    check(
        "malformed ID rejected",
        router
            .route(api, "GET", "/api/v3/datarefs/abc/value", "")
            .code
            == 400,
    );
    for text in [
        r#"{"data":"oops"}"#,
        r#"{"data":1e999}"#,
        r#"{"data":[1,,2]}"#,
        r#"{"data":2,"data":3}"#,
    ] {
        check(
            "invalid scalar/JSON rejected",
            router
                .route(api, "PATCH", &path("xphost/test/value"), text)
                .code
                == 400,
        );
    }
    let commands = router.route(
        api,
        "GET",
        "/api/v3/commands?filter[name]=xphost/test/pulse",
        "",
    );
    let command = commands.body["data"][0]["id"].as_u64();
    check("command lookup", command.is_some());
    if let Some(id) = command {
        check(
            "command activation",
            router
                .route(
                    api,
                    "POST",
                    &format!("/api/v3/command/{id}/activate"),
                    r#"{"duration":0}"#,
                )
                .code
                == 200,
        );
    }
    check("flight-loop step", api.step(dt).is_ok());
    check(
        "404",
        router.route(api, "GET", "/api/v3/nope", "").code == 404,
    );
    let (tx, rx) = mpsc::channel();
    let target = path("xphost/test/value");
    let worker = thread::spawn(move || {
        queued(
            &tx,
            "PATCH".into(),
            target,
            r#"{"data":999}"#.into(),
            Duration::from_millis(20),
        )
    });
    let response = worker.join().unwrap();
    check("sim timeout HTTP 504", response.code == 504);
    drain(&rx, &mut router, api);
    check(
        "timed-out write cancelled",
        api.read("xphost/test/value").ok() == Some(json!(3.75)),
    );
    router.finish(api);
    println!("selftest done: {failures} failure(s)");
    failures
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_ids_and_url_decoding() {
        for v in ["", "-1", "+1", "abc", "0junk", "2147483648"] {
            assert_eq!(parse_id(v), None);
        }
        assert_eq!(parse_id("0"), Some(0));
        assert_eq!(decode("filter%5Bname%5D"), "filter[name]");
    }
    #[test]
    fn strict_json_payloads() {
        assert!(serde_json::from_str::<Patch>(r#"{"data":1,"data":2}"#).is_err());
        assert!(serde_json::from_str::<Patch>(r#"{"data":1,"other":2}"#).is_err());
        assert!(serde_json::from_str::<Activation>(r#"{"duration":1e999}"#).is_err());
    }
}

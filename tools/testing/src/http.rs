//! A loopback HTTP/1.1 server for tests that need a fake package index or
//! static file host.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

/// Request targets (path plus query) seen by a server, in arrival order.
pub type RequestLog = Arc<Mutex<Vec<String>>>;

/// A successful response: content type and body.
pub struct Reply {
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

/// Starts a server on an ephemeral loopback port. `respond` maps a request
/// target to a [`Reply`], or `None` for a 404. Returns the base URL and the
/// request log.
pub fn serve<F>(respond: F) -> std::io::Result<(String, RequestLog)>
where
    F: Fn(&str) -> Option<Reply> + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let log: RequestLog = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&log);
    let respond = Arc::new(respond);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            let seen = Arc::clone(&seen);
            let respond = Arc::clone(&respond);
            std::thread::spawn(move || answer(stream, &seen, &*respond));
        }
    });
    Ok((format!("http://127.0.0.1:{port}"), log))
}

/// Snapshot of the request targets seen so far.
pub fn requests(log: &RequestLog) -> Vec<String> {
    log.lock().map(|log| log.clone()).unwrap_or_default()
}

fn answer(mut stream: TcpStream, seen: &RequestLog, respond: &dyn Fn(&str) -> Option<Reply>) {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        match stream.read(&mut byte) {
            Ok(1) => head.push(byte[0]),
            _ => break,
        }
    }
    let text = String::from_utf8_lossy(&head).into_owned();
    let mut words = text.split_whitespace();
    let method = words.next().unwrap_or_default().to_owned();
    let target = words.next().unwrap_or_default().to_owned();
    if let Ok(mut log) = seen.lock() {
        log.push(target.clone());
    }
    let (status, kind, body) = match respond(&target) {
        Some(reply) => ("200 OK", reply.content_type, reply.body),
        None => ("404 Not Found", "text/plain", Vec::new()),
    };
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: {kind}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    if method != "HEAD" {
        let _ = stream.write_all(&body);
    }
}

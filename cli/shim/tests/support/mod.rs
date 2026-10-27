#![allow(dead_code)]

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use sha2::{Digest, Sha256};

#[derive(Clone)]
pub enum Reply {
    Ok(Vec<u8>),
    Truncated(Vec<u8>, usize),
    Status(u16),
}

pub struct Server {
    pub base: String,
    routes: Arc<Mutex<HashMap<String, Reply>>>,
    hits: Arc<AtomicUsize>,
}

impl Server {
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let base = format!("http://{}/releases", listener.local_addr().expect("addr"));
        let routes: Arc<Mutex<HashMap<String, Reply>>> = Arc::default();
        let hits = Arc::new(AtomicUsize::new(0));
        let (routes_in, hits_in) = (routes.clone(), hits.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let routes = routes_in.clone();
                hits_in.fetch_add(1, Ordering::SeqCst);
                std::thread::spawn(move || serve(stream, &routes));
            }
        });
        Self { base, routes, hits }
    }

    pub fn route(&self, path: &str, reply: Reply) {
        self.routes
            .lock()
            .expect("routes")
            .insert(format!("/releases{path}"), reply);
    }

    pub fn release(&self, version: &str, asset: &str, body: &[u8], sidecar_for: &[u8]) {
        self.route(&format!("/v{version}/{asset}"), Reply::Ok(body.to_vec()));
        self.route(
            &format!("/v{version}/{asset}.sha256"),
            Reply::Ok(format!("{}  {asset}\n", sha256_hex(sidecar_for)).into_bytes()),
        );
    }

    pub fn url(&self, version: &str, asset: &str) -> String {
        format!("{}/v{version}/{asset}", self.base)
    }

    pub fn hits(&self) -> usize {
        self.hits.load(Ordering::SeqCst)
    }
}

fn serve(stream: TcpStream, routes: &Mutex<HashMap<String, Reply>>) {
    let mut reader = BufReader::new(stream.try_clone().expect("clone"));
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let path = line.split_whitespace().nth(1).unwrap_or("").to_owned();
    loop {
        let mut header = String::new();
        match reader.read_line(&mut header) {
            Ok(0) | Err(_) => break,
            Ok(_) if header == "\r\n" => break,
            Ok(_) => {}
        }
    }
    let reply = routes
        .lock()
        .expect("routes")
        .get(&path)
        .cloned()
        .unwrap_or(Reply::Status(404));
    let mut out = stream;
    let _ = match reply {
        Reply::Ok(body) => respond(&mut out, 200, &body, body.len()),
        Reply::Truncated(body, claimed) => respond(&mut out, 200, &body, claimed),
        Reply::Status(code) => respond(&mut out, code, b"", 0),
    };
}

fn respond(out: &mut TcpStream, code: u16, body: &[u8], length: usize) -> std::io::Result<()> {
    write!(
        out,
        "HTTP/1.1 {code} X\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n"
    )?;
    out.write_all(body)?;
    out.flush()
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn dead_base() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    drop(listener);
    format!("http://{addr}/releases")
}

pub fn host_asset() -> (String, String) {
    let (os, arch) = dx_shim::protocol::host_platform();
    let platform = dx_shim::protocol::platform(os, arch).expect("supported host");
    (
        dx_shim::protocol::asset_name(platform),
        dx_shim::protocol::cache_file_name(platform),
    )
}

pub fn leftovers(dir: &std::path::Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|name| name.ends_with(".part"))
                .collect()
        })
        .unwrap_or_default()
}

use std::io::{Read, Write};
use std::net::TcpListener;

#[allow(dead_code)]
#[path = "petstore.rs"]
mod petstore;

const BODY: &str = r#"{"id":7,"name":"Rex","tag":"dog"}"#;

fn serve_once() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = listener.local_addr().expect("addr");
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut buffer = [0u8; 4096];
        let read = stream.read(&mut buffer).expect("read");
        let request = String::from_utf8_lossy(&buffer[..read]).into_owned();
        let status = if request.starts_with("GET /pets/7 ") { "200 OK" } else { "404 Not Found" };
        let body = if status == "200 OK" { BODY } else { "{}" };
        let response = format!(
            "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(response.as_bytes()).expect("write");
    });
    format!("http://{address}")
}

#[test]
fn get_pet_decodes_fixture_response() {
    let base = serve_once();
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime");
    let pet = runtime
        .block_on(petstore::Client::new(&base).get_pet("7"))
        .expect("get_pet")
        .into_inner();
    assert_eq!(pet.id, 7);
    assert_eq!(pet.name, "Rex");
    assert_eq!(pet.tag.as_deref(), Some("dog"));
}

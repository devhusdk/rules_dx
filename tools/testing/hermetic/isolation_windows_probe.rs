//! Runs inside the isolated account: `connect ADDR`, `loopback`, `serve`, `parent`, `read PATH` or `canon PATH`.
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::time::Duration;

fn connect(addr: &str) -> Result<(), String> {
    let addr: SocketAddr = addr.parse().map_err(|err| format!("bad address: {err}"))?;
    TcpStream::connect_timeout(&addr, Duration::from_secs(15))
        .map(|_| ())
        .map_err(|err| err.to_string())
}

fn loopback() -> Result<(), String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|err| err.to_string())?;
    let addr = listener.local_addr().map_err(|err| err.to_string())?;
    let mut client = TcpStream::connect(addr).map_err(|err| err.to_string())?;
    let (mut server, _) = listener.accept().map_err(|err| err.to_string())?;
    client.write_all(b"ping").map_err(|err| err.to_string())?;
    let mut buf = [0u8; 4];
    server.read_exact(&mut buf).map_err(|err| err.to_string())?;
    if &buf == b"ping" {
        Ok(())
    } else {
        Err("loopback echoed the wrong bytes".to_owned())
    }
}

fn serve() -> Result<(), String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|err| err.to_string())?;
    let port = listener.local_addr().map_err(|err| err.to_string())?.port();
    println!("{port}");
    std::io::stdout().flush().map_err(|err| err.to_string())?;
    let (mut stream, _) = listener.accept().map_err(|err| err.to_string())?;
    stream.write_all(b"pong").map_err(|err| err.to_string())
}

fn parent() -> Result<(), String> {
    let me = std::env::current_exe().map_err(|err| err.to_string())?;
    let mut child = Command::new(me)
        .arg("serve")
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|err| format!("cannot start the server child: {err}"))?;
    let mut line = String::new();
    let stdout = child.stdout.take().ok_or("no child stdout")?;
    BufReader::new(stdout)
        .read_line(&mut line)
        .map_err(|err| err.to_string())?;
    let port: u16 = line
        .trim()
        .parse()
        .map_err(|_| format!("child said {line:?}"))?;
    let mut stream =
        TcpStream::connect(("127.0.0.1", port)).map_err(|err| format!("parent connect: {err}"))?;
    let mut reply = String::new();
    stream
        .read_to_string(&mut reply)
        .map_err(|err| err.to_string())?;
    let _ = child.wait();
    if reply == "pong" {
        Ok(())
    } else {
        Err(format!("child replied {reply:?}"))
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["connect", addr] => connect(addr),
        ["loopback"] => loopback(),
        ["serve"] => serve(),
        ["parent"] => parent(),
        ["read", path] => std::fs::read_to_string(path)
            .map(|text| println!("{text}"))
            .map_err(|err| err.to_string()),
        ["canon", path] => std::fs::canonicalize(path)
            .map(|real| println!("{}", real.display()))
            .map_err(|err| err.to_string()),
        _ => Err(
            "usage: connect ADDR | loopback | serve | parent | read PATH | canon PATH".to_owned(),
        ),
    };
    match result {
        Ok(()) => println!("ok"),
        Err(err) => {
            eprintln!("error: {err}");
            std::process::exit(1);
        }
    }
}

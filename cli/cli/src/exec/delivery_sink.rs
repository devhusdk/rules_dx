use std::io::{self, Write};

#[derive(Clone, Copy)]
pub(crate) enum Fault {
    None,
    WriteFails(io::ErrorKind),
    FlushFails(io::ErrorKind),
    OneByteWrites,
}

pub(crate) struct Sink {
    fault: Fault,
    fail_from_write: usize,
    writes: usize,
    pub(crate) bytes: Vec<u8>,
}

impl Sink {
    pub(crate) fn new(fault: Fault, fail_from_write: usize) -> Self {
        Sink {
            fault,
            fail_from_write,
            writes: 0,
            bytes: Vec::new(),
        }
    }

    pub(crate) fn text(&self) -> String {
        String::from_utf8_lossy(&self.bytes).into_owned()
    }

    pub(crate) fn names(&self) -> Vec<String> {
        self.text()
            .lines()
            .map(|line| {
                let value: serde_json::Value = serde_json::from_str(line).expect("event line");
                value["event"].as_str().expect("event name").to_owned()
            })
            .collect()
    }
}

impl Write for Sink {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        match self.fault {
            Fault::WriteFails(kind) if self.writes >= self.fail_from_write => {
                Err(io::Error::from(kind))
            }
            Fault::OneByteWrites if !buf.is_empty() => {
                self.bytes.push(buf[0]);
                Ok(1)
            }
            _ => {
                self.bytes.extend_from_slice(buf);
                Ok(buf.len())
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.fault {
            Fault::FlushFails(kind) => Err(io::Error::from(kind)),
            _ => Ok(()),
        }
    }
}

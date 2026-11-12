//! A scripted runner and the helpers the tests share.
#![expect(dead_code, reason = "each test uses a subset of the helpers")]

use std::cell::RefCell;
use std::collections::HashMap;

use dx_apple_ops::util::{Cmd, Output, Runner};

type Handler = Box<dyn Fn(&Cmd) -> Output>;

pub struct Fake {
    pub calls: RefCell<Vec<Cmd>>,
    handler: Handler,
}

impl Fake {
    pub fn new(handler: impl Fn(&Cmd) -> Output + 'static) -> Fake {
        Fake {
            calls: RefCell::new(Vec::new()),
            handler: Box::new(handler),
        }
    }

    /// The recorded calls as `args` joined by spaces.
    pub fn log(&self) -> Vec<String> {
        self.calls
            .borrow()
            .iter()
            .map(|cmd| cmd.args.join(" "))
            .collect()
    }
}

impl Runner for Fake {
    fn capture(&self, cmd: &Cmd) -> Output {
        self.calls.borrow_mut().push(cmd.clone());
        (self.handler)(cmd)
    }

    fn inherit(&self, cmd: &Cmd) -> Option<i32> {
        self.calls.borrow_mut().push(cmd.clone());
        (self.handler)(cmd).code
    }
}

pub fn ok(stdout: &str) -> Output {
    Output {
        code: Some(0),
        stdout: stdout.to_owned(),
        stderr: String::new(),
    }
}

pub fn fail(code: i32, stderr: &str) -> Output {
    Output {
        code: Some(code),
        stdout: String::new(),
        stderr: stderr.to_owned(),
    }
}

pub fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

pub fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<String, String> = pairs
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect();
    move |name| map.get(name).cloned().filter(|value| !value.is_empty())
}

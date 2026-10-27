#[derive(Debug, Clone, PartialEq)]
pub struct JunitCase {
    pub name: String,
    pub classname: Option<String>,
    pub time: f64,
    pub failure: Option<JunitMessage>,
    pub error: Option<JunitMessage>,
    pub skipped: Option<JunitMessage>,
    pub system_out: Option<String>,
    pub system_err: Option<String>,
    pub shard: u32,
    pub attempt: u32,
    pub run: u32,
    pub configuration: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JunitMessage {
    pub message: Option<String>,
    pub text: String,
}

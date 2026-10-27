#[derive(Debug, Clone, PartialEq)]
pub struct JunitCase {
    pub name: String,
    pub classname: Option<String>,
    pub(crate) time: f64,
    pub failure: Option<JunitMessage>,
    pub error: Option<JunitMessage>,
    pub skipped: Option<JunitMessage>,
    pub(crate) system_out: Option<String>,
    pub(crate) system_err: Option<String>,
    pub(crate) shard: u32,
    pub(crate) attempt: u32,
    pub run: u32,
    pub configuration: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JunitMessage {
    pub message: Option<String>,
    pub text: String,
}

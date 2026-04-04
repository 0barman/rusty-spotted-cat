pub enum ECode {
    Unknown = -1,
    /// 成功
    Success = 0,
    /// e
    RuntimeCreationFailedError = 101,

    ParameterEmpty = 102,

    /// the same file is already queued or running
    DuplicateTaskError = 103,
    EnqueueError = 104,

    IoError = 105,
    HttpError = 106,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    /// [ECode]
    code: i32,
    msg: String,
}

impl Error {
    pub fn new(code: i32, msg: String) -> Self {
        Error { code, msg }
    }

    pub fn code(&self) -> i32 {
        self.code
    }

    pub fn msg(&self) -> String {
        self.msg.clone()
    }

    pub fn from_code1(code: ECode) -> Self {
        Error {
            code: code as i32,
            msg: String::new(),
        }
    }

    pub fn from_code(code: ECode, msg: String) -> Self {
        Error {
            code: code as i32,
            msg,
        }
    }

    pub fn from_code_str(code: ECode, msg: &str) -> Self {
        Error {
            code: code as i32,
            msg: msg.to_string(),
        }
    }
}

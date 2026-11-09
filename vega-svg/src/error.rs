use std::fmt;

#[derive(Debug, Clone)]
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Spec,
    Data,
    Expression,
    Render,
    Io,
}

impl Error {
    pub fn spec(msg: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Spec,
            message: msg.into(),
        }
    }
    pub fn data(msg: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Data,
            message: msg.into(),
        }
    }
    pub fn expression(msg: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Expression,
            message: msg.into(),
        }
    }
    pub fn render(msg: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Render,
            message: msg.into(),
        }
    }
    pub fn io(msg: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Io,
            message: msg.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for Error {}

impl From<String> for Error {
    fn from(e: String) -> Self {
        Error::expression(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::io(e.to_string())
    }
}

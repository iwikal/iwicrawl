use failure::Fail;

#[derive(Debug, Fail)]
pub struct Error {
    message: String,
}

impl Error {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

impl Into<i32> for Error {
    fn into(self) -> i32 {
        1
    }
}

use core::fmt;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

pub fn recursive_cause(f: &dyn Fail) -> String {
    match f.cause() {
        Some(cause) => format!("{}: {}", f, recursive_cause(cause)),
        None => format!("{}", f),
    }
}

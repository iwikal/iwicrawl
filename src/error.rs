use failure::Fail;

#[derive(Debug, Fail)]
pub struct CliError(pub String);

impl Into<i32> for CliError {
    fn into(self) -> i32 {
        1
    }
}

use core::fmt;

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub fn recursive_cause(f: &dyn Fail) -> String {
    match f.cause() {
        Some(cause) => format!("{}: {}", f, recursive_cause(cause)),
        None => format!("{}", f),
    }
}

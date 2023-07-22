use std::fmt::{Debug, Formatter};

pub enum CliError {
    CreateRuntimeError(std::io::Error),
}

impl Debug for CliError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::CreateRuntimeError(e) => write!(f, "Cannot create tokio runtime: {e}"),
        }
    }
}

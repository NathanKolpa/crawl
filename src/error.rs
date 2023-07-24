use std::error::Error;
use std::fmt::{Debug, Display, Formatter};

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

pub enum CrawlerError {
    CannotSendRequest(reqwest::Error),
}

impl Display for CrawlerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CrawlerError::CannotSendRequest(e) => {
                let url = e.url().map(|x| x.as_str()).unwrap_or("[unknown]");

                write!(f, "Failed to send request to {url}")?;

                if let Some(inner) = e.source().and_then(|x| x.source()) {
                    write!(f, " ({inner})")?;
                }
            }
        }

        Ok(())
    }
}

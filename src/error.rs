use std::error::Error;
use std::fmt::{Display, Formatter};

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

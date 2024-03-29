use std::str::FromStr;

use crate::error::CrawlerError;
use html5ever::tendril::StrTendril;
use html5ever::tokenizer::{BufferQueue, StartTag, Token, TokenSink, TokenSinkResult, Tokenizer};
use reqwest::Client;
use url::Url;

use crate::filter::CrawledUrl;

struct LinkSink<'a> {
    parent: &'a CrawledUrl,
    links: Vec<CrawledUrl>,
}

impl TokenSink for LinkSink<'_> {
    type Handle = ();

    fn process_token(&mut self, token: Token, _line_number: u64) -> TokenSinkResult<Self::Handle> {
        let mut add_url = |s: &str| {
            let url = Url::options().base_url(Some(&self.parent.url)).parse(s);

            if let Ok(url) = url {
                match url.scheme() {
                    "http" | "https" => self.links.push(self.parent.push_new(url)),
                    _ => {}
                }
            }
        };

        if let Token::TagToken(tag) = token {
            if tag.kind == StartTag && (tag.name.eq("a") || tag.name.eq("atom:link")) {
                if let Some(href) = tag.attrs.into_iter().find(|x| x.name.local.eq("href")) {
                    add_url(href.value.to_string().as_str());
                }
            }
        }

        TokenSinkResult::Continue
    }
}

#[derive(Clone)]
pub struct LinkFetcher<'a> {
    client: &'a Client,
}

impl<'a> LinkFetcher<'a> {
    pub fn new(client: &'a Client) -> Self {
        Self { client }
    }
}

impl LinkFetcher<'_> {
    pub async fn fetch_links(&self, url: &CrawledUrl) -> Result<Vec<CrawledUrl>, CrawlerError> {
        let res = self
            .client
            .get(url.url.as_str())
            .send()
            .await
            .map_err(CrawlerError::CannotSendRequest)?;

        if let Some(content_type) = res
            .headers()
            .get("Content-Type")
            .and_then(|s| s.to_str().ok())
        {
            match content_type {
                s if s.starts_with("text/html") => {}
                s if s.starts_with("application/html") => {}
                s if s.starts_with("application/xml") => {}
                s if s.starts_with("text/xml") => {}
                _ => return Ok(Vec::new()),
            }
        }

        let body = res.text().await.map_err(CrawlerError::CannotSendRequest)?;

        let links = LinkSink {
            links: Default::default(),
            parent: url,
        };

        let mut body_queue = BufferQueue::new();
        body_queue.push_back(StrTendril::from_str(&body).unwrap());

        let mut tokenizer = Tokenizer::new(links, Default::default());
        let _ = tokenizer.feed(&mut body_queue);
        tokenizer.end();

        Ok(tokenizer.sink.links)
    }
}

use std::str::FromStr;

use crate::error::CrawlerError;
use html5ever::tendril::StrTendril;
use html5ever::tokenizer::{BufferQueue, StartTag, Token, TokenSink, TokenSinkResult, Tokenizer};
use reqwest::Client;
use url::Url;

use crate::rules::{CrawledUrl, CrawlerRules};

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

        match token {
            Token::TagToken(tag) => {
                if tag.kind == StartTag && (tag.name.eq("a") || tag.name.eq("atom:link")) {
                    if let Some(href) = tag.attrs.into_iter().find(|x| x.name.local.eq("href")) {
                        add_url(href.value.to_string().as_str());
                    }
                }
            }
            _ => {}
        }

        TokenSinkResult::Continue
    }
}

#[derive(Clone)]
pub struct Crawler<'a> {
    rules: CrawlerRules<'a>,
    client: &'a Client,
}

impl<'a> Crawler<'a> {
    pub fn new(rules: CrawlerRules<'a>, client: &'a Client) -> Self {
        Self { rules, client }
    }
}

impl Crawler<'_> {
    pub async fn crawl_url(&self, url: &CrawledUrl) -> Result<Vec<CrawledUrl>, CrawlerError> {
        let res = self
            .client
            .get(url.url.as_str())
            .send()
            .await
            .map_err(|e| CrawlerError::CannotSendRequest(e))?;

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
                s => return Err(CrawlerError::InvalidContentType(s.to_string())),
            }
        }

        let body = res
            .text()
            .await
            .map_err(|e| CrawlerError::CannotSendRequest(e))?;

        let links = LinkSink {
            links: Default::default(),
            parent: url,
        };

        let mut body_queue = BufferQueue::new();
        body_queue.push_back(StrTendril::from_str(&body).unwrap());

        let mut tokenizer = Tokenizer::new(links, Default::default());
        let _ = tokenizer.feed(&mut body_queue);
        tokenizer.end();

        let mut links = tokenizer.sink.links;
        links.retain(|x| self.rules.matches(&x.url));
        Ok(links)
    }
}

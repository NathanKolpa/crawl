use url::Url;

pub struct CrawledUrl {
    pub url: Url,
    pub depth: u32,
    pub origin_depth: u32,
}

impl CrawledUrl {
    pub fn push_new(&self, new: Url) -> Self {
        let is_other_origin = self.url.origin() != new.origin();

        Self {
            url: new,
            depth: self.depth + 1,
            origin_depth: self.origin_depth + is_other_origin as u32,
        }
    }
}

#[derive(Clone)]
pub struct CrawlerRules<'a> {
    pub parent_dirs: bool,
    pub ignore_robots: bool,
    pub max_origin_depth: Option<u32>,
    pub max_depth: Option<u32>,
    pub roots: &'a [Url],
}

impl CrawlerRules<'_> {
    pub fn matches(&self, url: &Url) -> bool {
        true
    }
}

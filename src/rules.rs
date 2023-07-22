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
    pub only_subdirs: bool,
    pub roots: &'a [Url],
}

impl CrawlerRules<'_> {
    pub fn matches(&self, url: &Url) -> bool {
        if self.only_subdirs {
            let is_subdir_of_root = self
                .roots
                .iter()
                .find(|root| {
                    root.scheme() == url.scheme()
                        && root.port() == url.port()
                        && root.host_str() == url.host_str()
                        && url.path().starts_with(root.path())
                })
                .is_some();

            if !is_subdir_of_root {
                return false;
            }
        }

        true
    }
}

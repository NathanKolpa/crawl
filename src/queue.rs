use std::collections::{HashMap, HashSet, VecDeque};

use url::{Origin, Url};

use crate::filter::CrawledUrl;

/// A data structure representing a queue of links that should be crawled.
pub struct CrawlerQueue {
    queue: VecDeque<CrawledUrl>,
    crawled: HashSet<Url>,
    allowed_by_robots: HashMap<Origin, bool>,
    require_robots: bool,
    robots_skip: usize,
    max_origin_depth: Option<u32>,
    max_depth: Option<u32>,
}

impl CrawlerQueue {
    pub fn new(
        require_robots: bool,
        max_origin_depth: Option<u32>,
        max_depth: Option<u32>,
    ) -> Self {
        Self {
            queue: Default::default(),
            crawled: Default::default(),
            allowed_by_robots: Default::default(),
            require_robots,
            robots_skip: 0,
            max_origin_depth,
            max_depth,
        }
    }

    pub fn check_and_queue_iter<I: Iterator<Item = CrawledUrl> + ExactSizeIterator>(
        &mut self,
        urls: I,
    ) {
        self.queue.reserve(urls.len());

        for url in urls {
            self.check_and_queue(url);
        }
    }

    pub fn check_and_queue(&mut self, url: CrawledUrl) {
        // TODO: we should check if the `url` has a shallower depth than the existing one.
        // If this is the case, do not re-queue, instead, update the exising ones
        if self.crawled.contains(&url.url) {
            return;
        }

        if self.require_robots {
            if let Some(false) = self.allowed_by_robots.get(&url.url.origin()).cloned() {
                return;
            }
        }

        if let Some(max_depth) = self.max_depth {}

        self.crawled.insert(url.url.clone());
        self.queue.push_back(url);
    }

    pub fn set_allowed_by_robots(&mut self, origin: Origin, allowed: bool) {
        self.allowed_by_robots.insert(origin, allowed);
        self.robots_skip = 0;
    }

    pub fn is_empty(&self) -> bool {
        if !self.require_robots {
            return self.queue.is_empty();
        }

        todo!()
    }

    pub fn take(&mut self) -> Option<CrawledUrl> {
        if !self.require_robots {
            return self.queue.pop_front();
        }

        while let Some(front) = self.queue.get(self.robots_skip) {
            match self.allowed_by_robots.get(&front.url.origin()).cloned() {
                Some(true) => {
                    // We found a valid match.
                    return self.queue.remove(self.robots_skip);
                }
                Some(false) => {
                    // Remove but don't return, because it is not valid.
                    // We also dont increment `robots_skip` because removing is effectively the same.
                    let _ = self.queue.remove(self.robots_skip);
                }
                None => {
                    // We need to know if we're allowed by robots.txt, so we skip to the next one.
                    self.robots_skip += 1;
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_take_without_robots() {
        let mut queue = CrawlerQueue::new(false, None, None);

        let url = CrawledUrl {
            url: "https://google.com/search".parse().unwrap(),
            depth: 0,
            origin_depth: 0,
        };

        queue.check_and_queue(url);
        assert!(queue.take().is_some())
    }

    #[test]
    fn test_add_and_take_with_allowed_robots() {
        let mut queue = CrawlerQueue::new(true, None, None);
        queue.set_allowed_by_robots(
            "https://google.com/search".parse::<Url>().unwrap().origin(),
            true,
        );

        let url = CrawledUrl {
            url: "https://google.com/search".parse().unwrap(),
            depth: 0,
            origin_depth: 0,
        };

        queue.check_and_queue(url);
        assert!(queue.take().is_some())
    }

    #[test]
    fn test_add_take_set_allowed_robots_and_take_again() {
        let mut queue = CrawlerQueue::new(true, None, None);

        let url = CrawledUrl {
            url: "https://google.com/search".parse().unwrap(),
            depth: 0,
            origin_depth: 0,
        };

        queue.check_and_queue(url);
        assert!(queue.take().is_none());
        assert!(queue.take().is_none());

        queue.set_allowed_by_robots(
            "https://google.com/search".parse::<Url>().unwrap().origin(),
            true,
        );

        assert!(queue.take().is_some())
    }

    #[test]
    fn test_add_take_set_disallowed_robots_and_take_again() {
        let mut queue = CrawlerQueue::new(true, None, None);

        let url = CrawledUrl {
            url: "https://google.com/search".parse().unwrap(),
            depth: 0,
            origin_depth: 0,
        };

        queue.check_and_queue(url);
        assert!(queue.take().is_none());
        assert!(queue.take().is_none());

        queue.set_allowed_by_robots(
            "https://google.com/search".parse::<Url>().unwrap().origin(),
            false,
        );

        assert!(queue.take().is_none())
    }
}

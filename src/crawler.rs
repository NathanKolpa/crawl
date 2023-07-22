use std::cmp::min;
use std::collections::VecDeque;
use std::pin::pin;
use std::sync::atomic::{AtomicUsize, Ordering};

use futures::future::{join_all, select_all};
use futures::join;
use tokio::select;
use tokio::sync::{broadcast, mpsc, oneshot};
use url::Url;

use crate::rules::{CrawledUrl, CrawlerRules};

pub enum CrawlerMessage<'a> {
    UrlFound(&'a Url),
}

enum MasterMessage {
    NewUrl(CrawledUrl)
}

enum WorkerMessage {
    Success(Vec<CrawledUrl>),
}

struct WorkerHandle {
    tx: mpsc::Sender<MasterMessage>,
    rx: mpsc::Receiver<WorkerMessage>,
    is_working: bool,
}


pub enum CrawlError {}

pub struct Crawler<'a, T> {
    rules: CrawlerRules<'a>,
    queue: VecDeque<CrawledUrl>,
    callback: T,
    max_jobs: usize,
}

impl<'a, T> Crawler<'a, T> {
    pub fn new(rules: CrawlerRules<'a>, max_jobs: usize, callback: T) -> Self {
        Self {
            rules,
            queue: Default::default(),
            callback,
            max_jobs,
        }
    }
}

impl<T: FnMut(CrawlerMessage)> Crawler<'_, T> {
    async fn start_master(&mut self, mut workers: Vec<WorkerHandle>) {
        println!("Master: starting");

        let mut active_workers = 0;

        while self.queue.len() > 0 || active_workers > 0 {
            // Dispatch a new job if there is work to do.
            if let Some(url) = self.queue.pop_back() {
                println!("Master: dispatching new job");

                // Do a linear scan for a free worker.
                let free_worker = workers.iter_mut().find(|x| !x.is_working);

                match free_worker {
                    None => {}
                    Some(worker) => {
                        println!("Master: dispatching to worker");
                        active_workers += 1;
                        worker.is_working = true;
                        worker.tx.send(MasterMessage::NewUrl(url)).await.unwrap();
                        println!("Master: dispatching complete");
                    }
                }
            }

            // Dispatching a new job won't do anything, instead we wait for messages.
            if (self.queue.len() == 0 || active_workers == self.max_jobs) && active_workers != 0 {
                println!("Master: waiting for message");
                let worker_recvs = workers.iter_mut()
                    .filter(|x| x.is_working)
                    .map(|x| {
                        Box::pin(x.rx.recv()) // TODO: can we remove this box pin?
                    });

                let (message, i, _remaining) = select_all(worker_recvs).await;
                drop(_remaining);
                let message = message.unwrap();

                println!("Master: received message");

                match message {
                    WorkerMessage::Success(urls) => {
                        workers[i].is_working = false;
                        active_workers -= 1;
                        self.queue.reserve(urls.len());

                        for url in urls {
                            self.queue.push_back(url);
                        }
                    }
                }
            }
        }

        println!("Master: done");
    }

    async fn crawl_url(url: &CrawledUrl) -> Result<Vec<CrawledUrl>, CrawlError> {
        let urls = Vec::new();

        Ok(urls)
    }

    async fn start_worker(tx: mpsc::Sender<WorkerMessage>, mut rx: mpsc::Receiver<MasterMessage>) {
        println!("Worker: starting");

        loop {
            match rx.recv().await {
                None => break,
                Some(MasterMessage::NewUrl(url)) => {
                    println!("Worker: started crawling");
                    let result = Self::crawl_url(&url).await;
                    println!("Worker: completed crawling");

                    match result {
                        Ok(urls) => tx.send(WorkerMessage::Success(urls)).await.unwrap(),
                        Err(_) => todo!()
                    }
                }
            }
        }

        println!("Worker: done");
    }

    pub async fn start(&mut self, roots: Vec<Url>) {
        self.queue.reserve(roots.len());

        for url in roots {
            self.queue.push_back(CrawledUrl {
                url,
                depth: 0,
                origin_depth: 0,
            });
        }

        let workers = (0..self.max_jobs).map(|_| {
            let (master_tx, master_rx) = mpsc::channel::<MasterMessage>(1);
            let (worker_tx, worker_rx) = mpsc::channel::<WorkerMessage>(1);

            (WorkerHandle {
                is_working: false,
                rx: worker_rx,
                tx: master_tx,
            }, Self::start_worker(worker_tx, master_rx))
        });

        let mut worker_futures = Vec::with_capacity(workers.len());
        let mut worker_channels = Vec::with_capacity(workers.len());

        for worker in workers {
            worker_futures.push(worker.1);
            worker_channels.push(worker.0);
        }

        let worker_handle = join_all(worker_futures);
        let master_handle = self.start_master(worker_channels);

        join!(
            worker_handle,
            master_handle
        );
    }
}

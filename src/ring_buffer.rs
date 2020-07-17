use crate::prelude::*;

#[derive(Debug, Clone)]
pub struct RingBuffer {
    pub stats: Stats,

    options: Arc<RwLock<Options>>,

    pub buffer: std::collections::VecDeque<Arc<Request>>,

    pub grouped: Option<GroupedStats>,
}

// for now, assuming that size is number of requests.
// can add number of seconds later, maybe by splitting into two structs
impl RingBuffer {
    pub fn new(options: Arc<RwLock<Options>>) -> Result<Self, Error> {
        // temporarily lock options to extract storage_type
        let o = options.read().unwrap();
        // parse storage_type option into size and type
        let storage_type = o.storage_type()?;
        drop(o);

        let size = match storage_type {
            StorageType::Requests(size) | StorageType::Timed(size) => size,
        };

        let buffer = std::collections::VecDeque::<Arc<Request>>::with_capacity(size as usize);

        let stats = Stats::new();

        let grouped = Some(GroupedStats::new(Arc::clone(&options))?);

        Ok(Self {
            stats,
            options,
            buffer,
            grouped,
        })
    }

    // nasty bodge to create a nested RingBuffer in another one by copying
    // its properties. This needs to die and be something better.
    pub fn from(capacity: usize, options: Arc<RwLock<Options>>) -> Self {
        Self {
            stats: Stats::new(),
            buffer: std::collections::VecDeque::<Arc<Request>>::with_capacity(capacity),
            options,
            grouped: None,
        }
    }

    pub fn first(&self) -> Option<&Arc<Request>> {
        self.buffer.front()
    }

    // Clear out self.grouped, if we have one, and repopulate it.
    //
    // This is used when the grouping key changes.
    pub fn regroup(&mut self) -> Option<Result<(), Error>> {
        let grouped = self.grouped.as_mut()?;
        grouped.clear();

        for request in self.buffer.iter() {
            if let Err(e) = grouped.add(Arc::clone(&request)) {
                return Some(Err(e));
            }
        }

        Some(Ok(()))
    }

    pub fn push(&mut self, request: Arc<Request>) -> Result<(), Error> {
        self.stats.add_request(&request);
        self.buffer.push_back(request.clone());

        if let Some(ref mut grouped) = self.grouped {
            grouped.add(request)?;
        }

        Ok(())
    }

    pub fn cleanup(&mut self) -> Result<(), Error> {
        let o = self.options.read().unwrap();
        let s = o.storage_type()?;
        drop(o);

        match s {
            StorageType::Requests(size) => {
                while self.buffer.len() > (size as usize) {
                    self.pop();
                }
            }
            StorageType::Timed(seconds) => {
                // check if first hits are older than size (seconds)
                while let Some(f) = self.first() {
                    let first = chrono::DateTime::<chrono::Utc>::from(f.time);
                    let age = chrono::Utc::now() - first;

                    if (age.num_seconds() as u64) < seconds {
                        break;
                    }

                    self.pop();
                }
            }
        };

        Ok(())
    }

    pub fn pop(&mut self) -> Option<Arc<Request>> {
        match self.buffer.pop_front() {
            Some(request) => {
                self.stats.remove_request(&request);

                // remove from grouped stats as well, if we have any
                if let Some(ref mut grouped) = self.grouped {
                    grouped.remove(Arc::clone(&request));
                }

                Some(request)
            }
            None => panic!("popping unknown request"),
        }
    }
}

impl Ord for RingBuffer {
    // compare the stats of this RingBuffer against another one, using the
    // ordering defined in options.order (also stored in each RingBuffer)
    //
    // this is how we order the rows in the main display table.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use super::options::Order;

        let this = &self.stats.global;
        let other = &other.stats.global;

        let options = self.options.read().unwrap();
        match options.order {
            Order::Requests => this.requests.cmp(&other.requests),
            Order::Size => this.bytes.cmp(&other.bytes),
        }
    }
}

impl PartialOrd for RingBuffer {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

// these are fake; Rust seems to think we need them but we never
// actually compare RingBuffers directly (ordering compares inner stats)
// so we don't need them.
impl PartialEq for RingBuffer {
    fn eq(&self, _: &RingBuffer) -> bool {
        todo!()
    }
}
impl Eq for RingBuffer {}

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum StorageType {
    Requests(u64),
    Timed(u64),
}

#[derive(Eq, PartialEq, Debug, Hash, Clone)]
pub enum GroupKey {
    IpAddress(IpAddr),
    Referer(String),
    StatusCode(i64),
    URI(String),
    Username(String),
}

impl std::fmt::Display for GroupKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(width) = f.width() {
            match self {
                Self::IpAddress(i) => write!(f, "{:.width$}", i, width = width),
                Self::Referer(r) => write!(f, "{:.width$}", r, width = width),
                Self::StatusCode(s) => write!(f, "{:.width$}", s, width = width),
                Self::URI(u) => write!(f, "{:.width$}", u, width = width),
                Self::Username(u) => write!(f, "{:.width$}", u, width = width),
            }
        } else {
            match self {
                Self::IpAddress(i) => write!(f, "{}", i),
                Self::Referer(r) => write!(f, "{}", r),
                Self::StatusCode(s) => write!(f, "{}", s),
                Self::URI(u) => write!(f, "{}", u),
                Self::Username(u) => write!(f, "{}", u),
            }
        }
    }
}

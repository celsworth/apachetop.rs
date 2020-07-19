use crate::prelude::*;

#[derive(Debug, Clone)]
pub struct RingBuffer {
    pub stats: Stats,

    options: Arc<Mutex<Options>>,

    pub buffer: VecDeque<Arc<Request>>,

    pub grouped: Option<GroupedStats>,
}

impl RingBuffer {
    pub fn new(options: Arc<Mutex<Options>>, with_grouped: bool) -> Result<Self, Error> {
        let size = Self::size_from_options(&options)?;

        // This is false when creating nested RingBuffers inside other GroupedStats, ie
        // we only support one level of grouping right now.
        //
        // Eventually it may be true but we'll need a circuit breaker to stop recursing
        // forever.
        let grouped = if with_grouped {
            // TODO: allow setting this per level in UI
            let group_by = options.lock().unwrap().group;

            Some(GroupedStats::new(Arc::clone(&options), group_by))
        } else {
            None
        };

        Ok(Self {
            stats: Stats::new(),
            options,
            buffer: VecDeque::<Arc<Request>>::with_capacity(size as usize),
            grouped,
        })
    }

    pub fn first(&self) -> Option<&Arc<Request>> {
        self.buffer.front()
    }

    // Clear out self.grouped, if we have one, and repopulate it according
    // to the new passed-in GroupBy.
    //
    // This is used when the grouping key changes.
    pub fn regroup(&mut self, group_by: GroupBy) -> Option<Result<(), Error>> {
        let grouped = self.grouped.as_mut()?;

        grouped.group_by(group_by);

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
        let o = self.options.lock().unwrap();
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

    fn size_from_options(options: &Arc<Mutex<Options>>) -> Result<u64, Error> {
        let o = options.lock().unwrap();
        Ok(match o.storage_type()? {
            StorageType::Requests(size) => size,
            StorageType::Timed(size) => size * 10, // assume 10 reqs/sec as a starting point
        })
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

        let options = self.options.lock().unwrap();
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

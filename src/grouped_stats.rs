use crate::prelude::*;

#[derive(Debug, Clone)]
pub struct GroupedStats {
    options: Arc<RwLock<Options>>,

    group_by: GroupBy,

    buffer: std::collections::HashMap<GroupKey, RingBuffer>,
}

impl GroupedStats {
    pub fn new(options: Arc<RwLock<Options>>, group_by: GroupBy) -> Self {
        let buffer = std::collections::HashMap::new();

        Self {
            options,
            group_by,
            buffer,
        }
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    // better in Request?
    pub fn group_key(&self, request: &Request) -> GroupKey {
        match self.group_by {
            GroupBy::IpAddress => GroupKey::IpAddress(request.ip_address),
            GroupBy::Referer => GroupKey::Referer(request.referer.clone()),
            GroupBy::Username => match request.username {
                Some(ref x) => GroupKey::Username(x.clone()),
                None => GroupKey::Username(String::new()),
            },
            GroupBy::StatusCode => GroupKey::StatusCode(request.status_code),
            GroupBy::URI => GroupKey::URI(request.uri.clone()),
        }
    }

    pub fn add(&mut self, request: Arc<Request>) -> Result<(), Error> {
        let key = self.group_key(&request);

        match self.buffer.get_mut(&key) {
            Some(bucket) => bucket.push(request)?,
            None => {
                // nest a new RingBuffer inside
                let mut bucket = RingBuffer::new(Arc::clone(&self.options), false)?;
                bucket.push(request)?;
                self.buffer.insert(key, bucket);
            }
        }

        Ok(())
    }

    pub fn remove(&mut self, request: Arc<Request>) {
        // if the first request in any hash value matches, pop it
        for v in self.buffer.values_mut() {
            if let Some(r) = v.buffer.front() {
                if r == &request {
                    v.pop();
                }
            }
        }

        // clean out hash entries which have no requests left
        self.buffer.retain(|_, v| !v.buffer.is_empty());
    }

    pub fn iter(&self) -> std::collections::hash_map::Iter<'_, GroupKey, RingBuffer> {
        self.buffer.iter()
    }
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

use crate::prelude::*;

#[derive(Debug, Clone)]
pub struct GroupedStats {
    options: Arc<RwLock<Options>>,

    buffer: std::collections::HashMap<GroupKey, RingBuffer>,
}

impl GroupedStats {
    pub fn new(options: Arc<RwLock<Options>>) -> Result<Self, Error> {
        let buffer = std::collections::HashMap::new();

        Ok(Self { options, buffer })
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    // better in Request?
    pub fn group_key(&self, request: &Request) -> GroupKey {
        use super::options::GroupBy;

        let options = self.options.read().unwrap();

        match options.group {
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
            Some(data) => data.push(request)?,
            None => {
                // nest a new RingBuffer inside
                let mut n = RingBuffer::from(self.buffer.len(), Arc::clone(&self.options));
                n.push(request)?;
                self.buffer.insert(key, n);
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

use crate::prelude::*;

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct Counters {
    pub requests: i64,
    pub bytes: i64,
}

impl Counters {
    pub fn empty() -> Self {
        Self {
            requests: 0,
            bytes: 0,
        }
    }

    pub fn add_request(&mut self, request: &Request) {
        self.requests += 1;
        self.bytes += request.size;
    }

    pub fn remove_request(&mut self, request: &Request) {
        self.requests -= 1;
        self.bytes -= request.size;
    }
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct Stats {
    pub global: Counters,

    // stats for 200-299 etc
    pub by_status_code: [Counters; 6],
}

impl Stats {
    pub fn new() -> Self {
        Self {
            global: Counters::empty(),
            by_status_code: [
                Counters::empty(),
                Counters::empty(),
                Counters::empty(),
                Counters::empty(),
                Counters::empty(),
                Counters::empty(),
            ],
        }
    }

    pub fn add_request(&mut self, request: &Request) {
        self.global.add_request(&request);

        // remove from appropriate HTTP status code Stats too
        let i = Self::status_code_stats_index_for_request(&request);
        let status_code_stats = &mut self.by_status_code[i];
        status_code_stats.add_request(&request);
    }

    pub fn remove_request(&mut self, request: &Request) {
        self.global.remove_request(&request);

        // remove from appropriate HTTP status code Stats too
        let i = Self::status_code_stats_index_for_request(&request);
        let status_code_stats = &mut self.by_status_code[i];
        status_code_stats.remove_request(&request);
    }

    fn status_code_stats_index_for_request(request: &Request) -> usize {
        match request.status_code {
            100..=199 => 1,
            200..=299 => 2,
            300..=399 => 3,
            400..=499 => 4,
            500..=599 => 5,
            _ => 0, // FIXME?
        }
    }
}

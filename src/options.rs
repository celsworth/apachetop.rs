use crate::prelude::*;

use structopt::StructOpt;

pub struct Wrapper(Arc<RwLock<Option<Options>>>);

pub fn get_options() -> Wrapper {
    Wrapper(OPTIONS.clone())
}

impl Wrapper {
    pub fn set(&self, options: Options) {
        self.write().replace(options);
    }

    pub fn read(&self) -> std::sync::RwLockReadGuard<Option<Options>> {
        self.0.read().unwrap()
    }

    pub fn write(&self) -> std::sync::RwLockWriteGuard<Option<Options>> {
        self.0.write().unwrap()
    }

    //pub fn foo(&self) -> Arc<Options> {
    //    Arc::new(self.read().unwrap())
    //}
}

#[derive(Debug, StructOpt)]
#[structopt()]
pub struct Options {
    /// Display refresh interval (seconds)
    #[structopt(short, long, default_value = "2")]
    pub interval: f64,

    /// Main table sort order column
    ///
    /// Can be either requests or size.
    #[structopt(short, long, default_value = "requests")]
    pub order: Order, // see bottom of file

    /// Group requests
    ///
    /// Determines how to aggregate statistics. By default the request URI
    /// is used.
    ///
    /// Can be: ip, referer, status, uri, username
    ///
    #[structopt(short, long, default_value = "uri")]
    pub group: GroupBy,

    /// Recent buffer size
    ///
    /// This should be an integer, optionally suffixed by s, m, h, or d.
    ///
    /// No suffix at all will store the given number of requests.
    ///
    /// A suffix of s stores requests for <size> seconds. m stores for <size> minutes,
    /// h is hours, and d is days.
    #[structopt(short = "s", long = "size", default_value = "1h")]
    pub buffer_size: String,

    /// Output logfile (for debugging)
    #[structopt(short, long, default_value = "apachetop.log", parse(from_os_str))]
    pub debug: std::path::PathBuf,

    /// Logfile(s) to open
    ///
    /// May be specified multiple times, and works with pipes.
    #[structopt(default_value = "/var/log/apache2/access.log", parse(from_os_str))]
    pub file: Vec<std::path::PathBuf>,
}

impl Options {
    pub fn new() -> Result<Self, Error> {
        let r = Self::from_args();

        if r.buffer_size.is_empty() {
            return Err(anyhow!("empty buffer size is invalid"));
        }

        Ok(r)
    }

    pub fn toggle_sort(&mut self) {
        self.order = match self.order {
            Order::Requests => Order::Size,
            Order::Size => Order::Requests,
        };
    }

    pub fn toggle_group(&mut self) {
        self.group = match self.group {
            GroupBy::IpAddress => GroupBy::Referer,
            GroupBy::Referer => GroupBy::StatusCode,
            GroupBy::StatusCode => GroupBy::URI,
            GroupBy::URI => GroupBy::Username,
            GroupBy::Username => GroupBy::IpAddress,
        };
    }

    // convert self.buffer_size into a tuple of (i64, ring_buffer::StorageType)
    pub fn storage_type(&self) -> Result<StorageType, Error> {
        let suffix = self.buffer_size.chars().last().unwrap();
        let x: &[_] = &['s', 'm', 'h', 'd'];
        let b = self.buffer_size.trim_end_matches(x);
        let size = b.parse::<u64>().context("failed to parse size")?;
        match suffix {
            's' => Ok(StorageType::Timed(size)),
            'm' => Ok(StorageType::Timed(size * 60)),
            'h' => Ok(StorageType::Timed(size * 3600)),
            'd' => Ok(StorageType::Timed(size * 86400)),
            _ => Ok(StorageType::Requests(size)),
        }
    }
}

#[derive(Eq, PartialEq, Debug)]
pub enum Order {
    Requests,
    Size,
}
impl std::str::FromStr for Order {
    type Err = Error;
    fn from_str(input: &str) -> std::result::Result<Self, <Self as std::str::FromStr>::Err> {
        // in actual fact, only a leading s is needed to order by size;
        // anything else is taken as requests
        match &input[0..1] {
            "S" | "s" => Ok(Self::Size),
            _ => Ok(Self::Requests),
        }
    }
}

#[derive(Eq, PartialEq, Debug)]
pub enum GroupBy {
    IpAddress,
    Referer,
    StatusCode,
    URI,
    Username,
}

// convert commandline args into a GroupBy object
impl std::str::FromStr for GroupBy {
    type Err = Error;
    fn from_str(input: &str) -> Result<Self, <Self as std::str::FromStr>::Err> {
        match input {
            "ip" => Ok(Self::IpAddress),
            "referer" | "referrer" => Ok(Self::Referer),
            "status" => Ok(Self::StatusCode),
            "username" => Ok(Self::Username),
            _ => Ok(Self::URI), // default and catchall
        }
    }
}

// used for display in table header
impl std::fmt::Display for GroupBy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IpAddress => write!(f, "IP"),
            Self::Referer => write!(f, "REFERER"),
            Self::StatusCode => write!(f, "CODE"),
            Self::URI => write!(f, "URI"),
            Self::Username => write!(f, "USERNAME"),
        }
    }
}

use crate::prelude::*;

use lazy_static::lazy_static;
use regex::Regex;
use strum_macros::EnumString;

#[derive(EnumString, Debug, Eq, PartialEq)]
pub enum HttpMethod {
    CONNECT,
    DELETE,
    GET,
    HEAD,
    OPTIONS,
    PATCH,
    POST,
    PUT,
}

#[derive(EnumString, Debug, Eq, PartialEq)]
pub enum HttpVersion {
    #[strum(serialize = "HTTP/0.9")]
    Http0_9,
    #[strum(serialize = "HTTP/1.0")]
    Http1_0,
    #[strum(serialize = "HTTP/1.1")]
    Http1_1,
    #[strum(serialize = "HTTP/2.0")]
    Http2_0,
    #[strum(serialize = "HTTP/3.0")]
    Http3_0,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Request {
    pub ip_address: IpAddr,
    pub identd: Option<String>,
    pub username: Option<String>,
    pub time: chrono::DateTime<chrono::FixedOffset>,
    pub method: HttpMethod,
    pub uri: String,
    pub http_version: HttpVersion,
    pub status_code: i64,
    pub size: i64,
    pub referer: String, // sic
    pub user_agent: String,
}

impl Request {
    pub fn new(input: &str) -> Result<Self, Error> {
        let r = Self::parse(input)?;

        let identd = match r.get(2).unwrap().as_str() {
            "-" => None,
            x => Some(String::from(x)),
        };

        let username = match r.get(3).unwrap().as_str() {
            "-" => None,
            x => Some(String::from(x)),
        };

        let time = r.get(4).unwrap().as_str();
        let time = chrono::DateTime::parse_from_str(time, "%d/%b/%Y:%T %z")?;

        Ok(Self {
            ip_address: r.get(1).unwrap().as_str().parse()?,
            identd,
            username,
            time,
            method: r.get(5).unwrap().as_str().parse()?,
            uri: String::from(r.get(6).unwrap().as_str()),
            http_version: r.get(7).unwrap().as_str().parse()?,
            status_code: r.get(8).unwrap().as_str().parse::<i64>()?,
            size: r
                .get(9)
                .unwrap()
                .as_str()
                .parse::<i64>()
                .unwrap_or_default(),
            referer: String::from(r.get(10).unwrap().as_str()),
            user_agent: String::from(r.get(11).unwrap().as_str()),
        })
    }

    fn parse(input: &str) -> Result<regex::Captures, Error> {
        lazy_static! {
            static ref CLF_RE: Regex = Regex::new(r#"^(\S+) (\S+) (\S+) \[([\w:/]+\s[+\-]\d{4})\] "(\S+)\s?(\S+)?\s?(\S+)?" (\d{3}|-) (\d+|-)\s?"?([^"]*)"?\s?"?([^"]*)?"?$"#).unwrap();
        }

        CLF_RE
            .captures(input)
            .ok_or_else(|| anyhow!("regex did not match input"))
    }
}

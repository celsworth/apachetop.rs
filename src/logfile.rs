use crate::prelude::*;

use std::io::prelude::*;

use crossbeam_channel::Sender;

use bstr::io::BufReadExt;

pub struct Logfile {
    pub path: std::path::PathBuf,
}

impl Logfile {
    pub fn new(path: std::path::PathBuf, request_tx: Sender<Request>) -> Result<Self, Error> {
        Self::start_reader(request_tx, &path)?;

        Ok(Logfile { path })
    }

    fn start_reader(
        request_tx: Sender<Request>,
        path: &std::path::PathBuf,
    ) -> Result<thread::JoinHandle<Result<(), Error>>, Error> {
        let mut fh = std::fs::File::open(&path)
            .with_context(|| format!("failed to open input logfile {}", &path.display()))?;

        // ignore failing seek result - this lets us operate on stdin/pipes
        let _ = fh.seek(std::io::SeekFrom::End(0));

        let mut br = std::io::BufReader::new(fh);

        let c = move || -> Result<_, _> {
            let sleep = std::time::Duration::from_millis(100);

            loop {
                br.by_ref().for_byte_line(|input| {
                    let line = String::from_utf8(input.to_vec());
                    if line.is_err() {
                        error!("logline not UTF-8: {:?}", line);
                        return Ok(true);
                    }

                    let line = line.unwrap();
                    //debug!("logline: {}", line);

                    match Request::new(&line) {
                        Ok(request) => {
                            request_tx.send(request).unwrap();
                        }
                        Err(e) => {
                            error!("unparseable logline ({}) :: {}", e, line);
                        }
                    }

                    Ok(true)
                })?;

                thread::sleep(sleep);
            }
        };

        let thread_name = format!("log_reader ({})", &path.display());
        Ok(thread::Builder::new().name(thread_name).spawn(c)?)
    }
}

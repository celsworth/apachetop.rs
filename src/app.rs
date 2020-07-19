use crate::prelude::*;

use crossbeam_channel::{unbounded, Receiver, Sender};

pub struct App {
    window: Window,

    // passed to RingBuffer and Window, then unused here so far
    options: Arc<Mutex<Options>>,

    // input logfiles. not used yet but may want to close/cleanup?
    logfiles: Vec<Logfile>,
}

impl App {
    pub fn new() -> Result<Self, Error> {
        let mut options = Options::new()?;
        Self::setup_logger(&options.debug)?;

        debug!("{:?}", options);

        // TODO: should probably abort process when a thread crashes?
        log_panics::init();

        let (request_tx, request_rx): (Sender<Request>, Receiver<Request>) = unbounded();

        let logfiles = options
            .file
            .drain(..)
            .map(|file| Logfile::new(file, request_tx.clone()))
            .collect::<Result<_, _>>()?;

        let options = Arc::new(Mutex::new(options));
        let alltime_stats = Arc::new(Mutex::new(Stats::new()));
        let ring_buffer = Arc::new(Mutex::new(RingBuffer::new(Arc::clone(&options), true)?));
        Self::start_request_receiver(
            request_rx,
            Arc::clone(&alltime_stats),
            Arc::clone(&ring_buffer),
        )?;

        // do this last so any errors in setting up the rest of the app are displayed
        let window = Window::new(Arc::clone(&options), alltime_stats, ring_buffer);

        Ok(App {
            options,
            logfiles,
            window,
        })
    }

    fn setup_logger(path: &std::path::PathBuf) -> Result<(), fern::InitError> {
        fern::Dispatch::new()
            .format(|out, message, record| {
                out.finish(format_args!(
                    "{} {} {}",
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                    record.level(),
                    message
                ))
            })
            .level(log::LevelFilter::Debug)
            .chain(fern::log_file(path)?)
            .apply()?;

        Ok(())
    }

    pub fn start(&mut self) -> Result<(), Error> {
        self.window.run()?;
        Ok(())
    }

    // thread to receive Request methods from each Logfile instance
    // and append to alltime_stats and ring_buffer
    fn start_request_receiver(
        request_rx: Receiver<Request>,
        alltime_stats: Arc<Mutex<Stats>>,
        ring_buffer: Arc<Mutex<RingBuffer>>,
    ) -> Result<thread::JoinHandle<Result<(), Error>>, Error> {
        let c = move || {
            for request in request_rx {
                //debug!("Request is {:?}", request);

                {
                    let mut alltime_stats = alltime_stats.lock().unwrap();
                    alltime_stats.add_request(&request);
                }

                {
                    let mut ring_buffer = ring_buffer.lock().unwrap();
                    ring_buffer.push(Arc::new(request))?;
                }
            }

            Ok(())
        };

        Ok(thread::Builder::new()
            .name("request_rx".to_string())
            .spawn(c)?)
    }
}

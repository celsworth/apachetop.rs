use crate::prelude::*;

use crossterm::style::{Attribute, Print, Styler};
use crossterm::{cursor, terminal, ExecutableCommand, QueueableCommand};

use std::io::{stdout, Write};

const CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct Window {
    started_at: StartedAt,

    lines: u16,
    cols: u16,

    options: Arc<Mutex<Options>>,

    alltime_stats: Arc<Mutex<Stats>>,
    ring_buffer: Arc<Mutex<RingBuffer>>,
}

impl Window {
    pub fn new(
        options: Arc<Mutex<Options>>,
        alltime_stats: Arc<Mutex<Stats>>,
        ring_buffer: Arc<Mutex<RingBuffer>>,
    ) -> Self {
        let (cols, lines) = crossterm::terminal::size().unwrap();

        let now = std::time::Instant::now();
        Window {
            started_at: StartedAt(now),
            lines,
            cols,
            options,
            alltime_stats,
            ring_buffer,
        }
    }

    pub fn run(&mut self) -> Result<(), Error> {
        // temporary lock on options to get interval
        let options = self.options.lock().unwrap();
        let interval = options.interval;
        drop(options);
        // support f64 seconds by multiplying then using from_millis
        let interval = std::time::Duration::from_millis((interval * 1000.0) as u64);

        crossterm::terminal::enable_raw_mode()?;

        // stdout().execute(crossterm::event::EnableMouseCapture)?
        stdout().execute(cursor::Hide)?;
        stdout().execute(terminal::EnterAlternateScreen)?;
        stdout().execute(terminal::SetTitle("apachetop"))?;

        loop {
            self.redraw()?;

            if crossterm::event::poll(interval)? && !self.handle_event()? {
                break;
            }
        }

        crossterm::terminal::disable_raw_mode()?;

        stdout().execute(terminal::LeaveAlternateScreen)?;
        stdout().execute(cursor::Show)?;
        // stdout().execute(crossterm::event::DisableMouseCapture)?;

        Ok(())
    }

    fn redraw(&mut self) -> Result<(), Error> {
        let mut stdout = stdout();

        stdout
            .queue(terminal::Clear(terminal::ClearType::All))?
            .queue(cursor::MoveTo(0, 0))?
            .queue(Print(format!("apachetop {}", CARGO_PKG_VERSION)))?
            .queue(cursor::MoveTo(self.cols / 2, 0))?
            .queue(Print(self.started_at.to_string()))?
            .queue(cursor::MoveTo(self.cols - 8 as u16, 0))?
            .queue(Print(chrono::Local::now().format("%H:%M:%S").to_string()))?;

        {
            let alltime_stats = self.alltime_stats.lock().unwrap();
            let elapsed = self.started_at.elapsed().as_secs() as f64;

            stdout
                .queue(cursor::MoveTo(0, 1))?
                .queue(Print(self.primary_stats_line(
                    &alltime_stats,
                    elapsed,
                    true,
                )))?
                .queue(cursor::MoveTo(0, 2))?
                .queue(Print(self.per_code_line(&alltime_stats)))?;
        } // mutex on alltime_stats

        {
            let mut ring_buffer = self.ring_buffer.lock().unwrap();

            // TODO: better in another thread, not at display time?
            ring_buffer.cleanup()?;

            let elapsed = match ring_buffer.first() {
                Some(f) => {
                    let first = chrono::DateTime::<chrono::Utc>::from(f.time);
                    (chrono::Utc::now() - first).num_seconds() as f64
                }
                None => 1.0, // avoid divide by zero later
            };

            stdout
                .queue(cursor::MoveTo(0, 3))?
                .queue(Print(self.primary_stats_line(
                    &ring_buffer.stats,
                    elapsed,
                    false,
                )))?
                .queue(cursor::MoveTo(0, 4))?
                .queue(Print(self.per_code_line(&ring_buffer.stats)))?;

            {
                let options = self.options.lock().unwrap();
                stdout.queue(cursor::MoveTo(0, 6))?.queue(Print(
                    format!(
                        "{:width$}",
                        &format!(
                            "  REQS REQS/S    SIZE    SZ/S {}",
                            options.group.to_string()
                        ),
                        width = self.cols as usize
                    )
                    .negative(),
                ))?;
            } // read lock on options

            if let Some(grouped) = &ring_buffer.grouped {
                use lazysort::SortedBy;

                // convert HashMap<GroupKey, RingBuffer> to Vec<(GroupKey, RingBuffer)>,
                // sort it by the RingBuffers, then lazy-sort the first n lines for display.
                for (key, ring_buffer) in grouped
                    .iter()
                    .filter(|(_, v)| !v.buffer.is_empty()) // filter out empty buffers to save work
                    .collect::<Vec<(&GroupKey, &RingBuffer)>>()
                    .iter()
                    .sorted_by(|a, b| b.1.cmp(&a.1)) // see impl Ord for RingBuffer
                    .take((self.lines - 7/* lines used for header */) as usize)
                {
                    stdout
                        .queue(cursor::MoveToNextLine(1))?
                        .queue(Print(self.table_line(key, ring_buffer, elapsed)))?;
                }
            }
        } // mutex on ring_buffer

        stdout.flush()?;

        Ok(())
    }

    fn handle_event(&mut self) -> Result<bool, Error> {
        use crossterm::event::Event::{Key, Mouse, Resize};
        use crossterm::event::KeyCode::Char;
        use crossterm::event::{KeyEvent, KeyModifiers};

        match crossterm::event::read()? {
            Key(KeyEvent {
                code: Char('q'), ..
            })
            | Key(KeyEvent {
                modifiers: KeyModifiers::CONTROL,
                code: Char('c'),
            }) => return Ok(false),
            Key(KeyEvent {
                code: Char('o'), ..
            }) => {
                self.toggle_sort();
            }
            Key(KeyEvent {
                code: Char('g'), ..
            }) => {
                self.toggle_group();
            }

            Key(event) => info!("{:?}", event),
            Mouse(event) => info!("{:?}", event),
            Resize(cols, lines) => {
                self.lines = lines;
                self.cols = cols;
            }
        }

        Ok(true)
    }

    fn toggle_sort(&self) {
        self.options.lock().unwrap().toggle_sort();
    }

    fn toggle_group(&self) {
        let mut o = self.options.lock().unwrap();
        let group_by = o.toggle_group();
        drop(o);
        self.ring_buffer.lock().unwrap().regroup(group_by);
    }

    fn table_line(&self, key: &GroupKey, rr: &RingBuffer, elapsed: f64) -> String {
        let reqs = rr.stats.global.requests as f64;
        format!(
            "{reqs:6} {reqs_per_sec:6.2} {hb:>6} {hb_per_sec:>6} {key:width$}",
            width = (self.cols - 30) as usize,
            reqs = reqs,
            reqs_per_sec = reqs / elapsed,
            hb = Self::humansize(rr.stats.global.bytes as f64),
            hb_per_sec = Self::humansize(rr.stats.global.bytes as f64 / elapsed),
            key = key
        )
    }

    // All:       638924 reqs ( 182.65/sec)      3433539K ( 981.6K/sec)  (   5.4K/req)
    fn primary_stats_line(&self, stats: &Stats, elapsed: f64, alltime: bool) -> String {
        let reqs_non_zero = std::cmp::max(stats.global.requests, 1) as f64;
        let reqs = stats.global.requests as f64;

        let header = if alltime { "All:" } else { "R:" };

        format!(
            "{header:5} {bold}{reqs:>space$}{reset} ({reqs_per_sec:6.2}/sec) {bold}{hb:>space$}{reset} ({hb_per_sec}/sec) {hb_per_req}/req",
            bold = Attribute::Bold,
            reset =  Attribute::Reset,
            space = ((self.cols - 50) / 2) as usize,
            header = header,
            reqs = reqs,
            reqs_per_sec = reqs / elapsed,
            hb = Self::humansize(stats.global.bytes as f64),
            hb_per_sec = Self::humansize(stats.global.bytes as f64 / elapsed),
            hb_per_req = Self::humansize((stats.global.bytes as f64) / reqs_non_zero)
        )
    }

    // 2xx:  455415 (71.3%) 3xx:  175745 (27.5%) 4xx:  7746 ( 1.2%) 5xx:    10 ( 0.0%)
    fn per_code_line(&self, stats: &Stats) -> String {
        let stats_2 = &stats.by_status_code[2];
        let stats_3 = &stats.by_status_code[3];
        let stats_4 = &stats.by_status_code[4];
        let stats_5 = &stats.by_status_code[5];

        // closure to reduce some duplication for some munging below
        let c = |rb_stats: &crate::stats::Counters| -> (f64, usize) {
            // avoid divide by zero if there's no requests yet
            let pct = if stats.global.requests > 0 {
                100.0 * (rb_stats.requests as f64 / stats.global.requests as f64)
            } else {
                0.0
            };

            // intelligent dp detection: eg 2.34%, 10.5%, 100%
            let dp = if (pct - 100.0).abs() < f64::EPSILON {
                0
            } else if pct < 10.0 {
                2
            } else {
                1
            };

            (pct, dp)
        };

        let (code_2_pct, code_2_dp) = c(stats_2);
        let (code_3_pct, code_3_dp) = c(stats_3);
        let (code_4_pct, code_4_dp) = c(stats_4);
        let (code_5_pct, code_5_dp) = c(stats_5);

        format!(
            "2xx: {code_2:space$} ({code_2_pct:4.code_2_dp$}%) 3xx: {code_3:space$} ({code_3_pct:4.code_3_dp$}%) 4xx: {code_4:space$} ({code_4_pct:4.code_4_dp$}%) 5xx: {code_5:space$} ({code_5_pct:4.code_5_dp$}%)",
            space = ((self.cols - 55) / 4) as usize,
            code_2 = stats_2.requests,
            code_2_dp = code_2_dp,
            code_2_pct = code_2_pct,
            code_3 = stats_3.requests,
            code_3_dp = code_3_dp,
            code_3_pct = code_3_pct,
            code_4 = stats_4.requests,
            code_4_dp = code_4_dp,
            code_4_pct = code_4_pct,
            code_5 = stats_5.requests,
            code_5_dp = code_5_dp,
            code_5_pct = code_5_pct,
        )
    }

    fn humansize(bytes: f64) -> String {
        if bytes > 1073741824.0 {
            format!("{:6.2}G", (bytes / 1073741824.0))
        } else if bytes > 1048576.0 {
            format!("{:6.2}M", (bytes / 1048576.00))
        } else if bytes > 1024.0 {
            format!("{:6.2}K", (bytes / 1024.0))
        } else {
            format!("{:6.0}B", bytes)
        }
    }
}

struct StartedAt(std::time::Instant);

impl StartedAt {
    fn elapsed(&self) -> std::time::Duration {
        self.0.elapsed()
    }
}

impl std::fmt::Display for StartedAt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let duration = self.0.elapsed().as_secs();

        let hours = duration / 3600;
        let minutes = duration % 3600 / 60;
        let seconds = duration % 60;

        write!(f, "runtime: ")?;
        if hours > 0 {
            write!(f, "{}h ", hours)?;
        }
        if hours > 0 || minutes > 0 {
            write!(f, "{}m ", minutes)?;
        }
        write!(f, "{}s", seconds)
    }
}

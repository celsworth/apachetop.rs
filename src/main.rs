mod app;
mod grouped_stats;
mod logfile;
mod options;
mod prelude;
mod request;
mod ring_buffer;
mod stats;
mod window;

use crate::prelude::*;

use lazy_static::lazy_static;
lazy_static! {
    pub static ref OPTIONS: Arc<RwLock<Option<Options>>> = Arc::new(RwLock::new(None));
}

fn main() {
    if let Err(err) = try_main() {
        println!("Error: {:?}", err);
        std::process::exit(255);
    }
}

fn try_main() -> Result<(), Error> {
    let mut app = App::new()?;

    app.start()?;

    Ok(())
}

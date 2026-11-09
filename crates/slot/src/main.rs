#[cfg(not(feature = "host"))]
mod device_app;
#[cfg(feature = "host")]
mod host_app;

fn main() {
    // The command line is read once, into a named first argument, because every branch below
    // is a different program: `--init-root` and `--play` both return without ever opening a
    // window. Calling `next()` again per branch is how a second flag silently becomes the
    // first one's argument and the frontend starts instead.
    let mut args = std::env::args().skip(1);
    let first = args.next();

    // Build tooling needs the content layout without opening a window. Not a user setting:
    // the folders come from root::DIRS either way, so there is only one of them to diverge.
    if first.as_deref() == Some("--init-root") {
        let Some(dir) = args.next() else {
            eprintln!("slot: --init-root needs a directory");
            std::process::exit(2);
        };
        slot::root::ensure(std::path::Path::new(&dir));
        return;
    }

    // The card's music, through the device's own audio path, with no screen involved. A smoke
    // test rather than a feature: it is the only way to prove this build can decode a track and
    // be heard doing it, and the handheld has no console to prove it any other way.
    if first.as_deref() == Some("--play") {
        let Some(file) = args.next() else {
            eprintln!("slot: --play needs a file");
            std::process::exit(2);
        };
        let secs = args.next().and_then(|s| s.parse().ok()).unwrap_or(10.0);
        if let Err(e) = slot::music::smoke(std::path::Path::new(&file), secs) {
            eprintln!("slot: music: {e}");
            std::process::exit(1);
        }
        return;
    }

    // The deck, played on its own thread with the caller still running. What `smoke` cannot
    // show: that it plays at all while something else is happening, and that the tap carries
    // samples for a visualiser to read.
    if first.as_deref() == Some("--deck") {
        let Some(file) = args.next() else {
            eprintln!("slot: --deck needs a file");
            std::process::exit(2);
        };
        let secs = args.next().and_then(|s| s.parse().ok()).unwrap_or(6.0);
        if let Err(e) = slot::music::deck_probe(std::path::Path::new(&file), secs) {
            eprintln!("slot: deck: {e}");
            std::process::exit(1);
        }
        return;
    }

    #[cfg(feature = "host")]
    host_app::run();

    #[cfg(not(feature = "host"))]
    device_app::run();
}

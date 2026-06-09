use std::process;

mod cli;
mod conformance;
mod validate;

fn main() {
    // SIGPIPE safety: reset SIGPIPE to default so piping to head/less doesn't panic.
    // SAFETY: calling signal(SIGPIPE, SIG_DFL) is safe at process start before any
    // signal handlers have been installed. This is the standard pattern for Rust CLIs
    // per self_sigpipe_panic_toolkit.
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    if let Err(e) = cli::run() {
        eprintln!("error: {e:#}");
        process::exit(1);
    }
}

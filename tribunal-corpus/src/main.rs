use std::process;

mod cli;
mod validate;

fn main() {
    // SIGPIPE safety: reset SIGPIPE to default so piping to head/less doesn't panic
    #[cfg(unix)]
    // SAFETY: setting SIGPIPE to SIG_DFL is always safe at process start
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    if let Err(e) = cli::run() {
        eprintln!("error: {e:#}");
        process::exit(1);
    }
}

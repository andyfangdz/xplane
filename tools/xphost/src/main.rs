#![forbid(unsafe_code)]
use std::{
    fs,
    sync::{atomic::AtomicBool, Arc},
    time::Instant,
};
use xphost::{cli, script::Runner, serve};
use xphost_core::{Backend, FlightStart, Request};
#[cfg(windows)]
fn run() -> Result<i32, String> {
    let Some(options) = cli::from_env()? else {
        println!("{}", cli::USAGE);
        return Ok(0);
    };
    cli::prepare(&options)?;
    let fake = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("executable has no parent")?
        .join("XPLM_64.dll");
    let start = Instant::now();
    let mut host =
        match xphost_native::Host::load(options.config.clone(), &fake, options.navdat.as_deref()) {
            Ok(host) => host,
            Err(e) => {
                println!("xphost: {e}");
                return Ok(
                    if e.starts_with("plugin fault") || e.starts_with("plugin rejected") {
                        2
                    } else {
                        1
                    },
                );
            }
        };
    host.request(Request::FlightStart(if options.running {
        FlightStart::Running
    } else {
        FlightStart::Cold
    }))?;
    for id in [102, 103, 104] {
        host.send_message(id);
    }
    let stop = Arc::new(AtomicBool::new(false));
    let signal = stop.clone();
    ctrlc::set_handler(move || signal.store(true, std::sync::atomic::Ordering::Release))
        .map_err(|e| e.to_string())?;
    let mut failed = false;
    let mut faulted = false;
    let mut serve_error = None;
    if options.serve {
        if options.selftest {
            failed = serve::selftest(&host, options.dt) > 0;
        } else if let Err(e) = serve::run(&host, options.port, options.dt, options.fast, stop) {
            serve_error = Some(e);
        }
    } else {
        let mut runner = Runner::new(&host, options.dt, options.fast, options.out.clone(), stop);
        runner.debug_request = options.debug_request.clone();
        runner.debug_result = options.debug_result.clone();
        if let Some(script) = &options.script {
            runner.script(script);
        }
        let seconds = options
            .seconds
            .unwrap_or(if options.script.is_some() { 0. } else { 10. });
        runner.advance(seconds)?;
        failed = runner.failed;
        faulted = runner.faulted;
    }
    faulted |= host.shutdown();
    let stats = host.request(Request::Stats)?;
    faulted |= stats["faulted"].as_bool().unwrap_or(false);
    let misses = host.request(Request::Misses)?;
    fs::write(
        options.out.join("dataref-misses.txt"),
        misses.as_str().unwrap_or(""),
    )
    .map_err(|e| e.to_string())?;
    let sim = stats["sim_seconds"].as_f64().unwrap_or(0.);
    let wall = start.elapsed().as_secs_f64();
    println!(
        "xphost: sim {sim:.2} s in {wall:.2} s wall ({:.1}x, {} frames, dt {:.4})",
        sim / wall,
        stats["frames"],
        options.dt
    );
    println!("xphost: dataref reads sim={} plugin={}, writes sim={} plugin={}, commands={}, loops={}, find misses={}",stats["sim_reads"],stats["plugin_reads"],stats["sim_writes"],stats["plugin_writes"],stats["commands_fired"],stats["loops_run"],stats["find_misses"]);
    println!("xphost: log {}", options.out.join("Log.txt").display());
    if faulted {
        println!("xphost: PLUGIN FAULT (see Log.txt)");
        return Ok(2);
    }
    if let Some(error) = serve_error {
        return Err(error);
    }
    Ok(if failed { 3 } else { 0 })
}
#[cfg(not(windows))]
fn run() -> Result<i32, String> {
    Err("xphost requires Windows x64 MSVC".into())
}
fn main() {
    let code = match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("xphost: {error}");
            1
        }
    };
    std::process::exit(code);
}

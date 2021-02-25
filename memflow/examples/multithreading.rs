use std::thread;

use clap::*;
use log::{info, Level};

use memflow::error::{Error, ErrorKind, ErrorOrigin, Result};
use memflow::os::*;
use memflow::plugins::*;

// This function shows how the connector can be cloned.
// For each cloned connector a thread is spawned that initializes a seperate OS instance.
pub fn parallel_init(
    connector: ConnectorInstance,
    inventory: &Inventory,
    os_name: &str,
    os_args: &Args,
) {
    rayon::scope(|s| {
        (0..8).map(|_| connector.clone()).into_iter().for_each(|c| {
            s.spawn(move |_| {
                inventory.create_os(os_name, Some(c), os_args).unwrap();
            })
        })
    });
}

// This function shows how a kernel can be cloned.
// For each cloned kernel a thread is spawned that will iterate over all processes of the target in parallel.
pub fn parallel_kernels(kernel: OSInstance) {
    (0..8)
        .map(|_| kernel.clone())
        .into_iter()
        .map(|mut k| {
            thread::spawn(move || {
                let _eprocesses = k.process_address_list().unwrap();
            })
        })
        .for_each(|t| t.join().unwrap());
}

// This function shows how a process can be cloned.
// For each cloned process a thread is spawned that will iterate over all the modules of this process in parallel.
pub fn parallel_processes(kernel: OSInstance) {
    let process = kernel.into_process_by_name("wininit.exe").unwrap();

    (0..8)
        .map(|_| process.clone())
        .into_iter()
        .map(|mut p| {
            thread::spawn(move || {
                let module_list = p.module_list().unwrap();
                info!("wininit.exe module_list: {}", module_list.len());
            })
        })
        .for_each(|t| t.join().unwrap());
}

pub fn main() {
    let (conn_name, conn_args, os_name, os_args, log_level) = parse_args().unwrap();

    simple_logger::SimpleLogger::new()
        .with_level(log_level.to_level_filter())
        .init()
        .unwrap();

    // create inventory + connector
    let inventory = Inventory::scan();
    let connector = inventory
        .create_connector(&conn_name, None, &conn_args)
        .unwrap();

    // parallel test functions
    // see each function's implementation for further details

    // showcasing parallel initialization of kernel objects
    parallel_init(connector.clone(), &inventory, &os_name, &os_args);

    let kernel = inventory
        .create_os(&os_name, Some(connector), &os_args)
        .unwrap();

    // showcasing parallel process iteration
    parallel_kernels(kernel.clone());

    // showcasing parallel module iteration
    parallel_processes(kernel);
}

fn parse_args() -> Result<(String, Args, String, Args, log::Level)> {
    let matches = App::new("multithreading example")
        .version(crate_version!())
        .author(crate_authors!())
        .arg(Arg::with_name("verbose").short("v").multiple(true))
        .arg(
            Arg::with_name("connector")
                .long("connector")
                .short("c")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("conn-args")
                .long("conn-args")
                .short("x")
                .takes_value(true)
                .default_value(""),
        )
        .arg(
            Arg::with_name("os")
                .long("os")
                .short("o")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("os-args")
                .long("os-args")
                .short("y")
                .takes_value(true)
                .default_value(""),
        )
        .get_matches();

    // set log level
    let level = match matches.occurrences_of("verbose") {
        0 => Level::Error,
        1 => Level::Warn,
        2 => Level::Info,
        3 => Level::Debug,
        4 => Level::Trace,
        _ => Level::Trace,
    };

    Ok((
        matches
            .value_of("connector")
            .ok_or_else(|| {
                Error(ErrorOrigin::Other, ErrorKind::Configuration)
                    .log_error("failed to parse connector")
            })?
            .into(),
        Args::parse(matches.value_of("conn-args").ok_or_else(|| {
            Error(ErrorOrigin::Other, ErrorKind::Configuration)
                .log_error("failed to parse connector args")
        })?)?,
        matches
            .value_of("os")
            .ok_or_else(|| {
                Error(ErrorOrigin::Other, ErrorKind::Configuration).log_error("failed to parse os")
            })?
            .into(),
        Args::parse(matches.value_of("os-args").ok_or_else(|| {
            Error(ErrorOrigin::Other, ErrorKind::Configuration).log_error("failed to parse os args")
        })?)?,
        level,
    ))
}

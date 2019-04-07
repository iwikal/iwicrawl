#![warn(bare_trait_objects)]

#[macro_use]
extern crate log;
#[macro_use]
extern crate html5ever;

use clap::{App, Arg};
use url::Url;

mod error;
mod fetch;
mod tok;

fn main() {
    let matches = App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .arg(
            Arg::with_name("verbosity")
                .short("v")
                .long("verbose")
                .multiple(true)
                .help("Increase message verbosity for each occurrance of the flag"),
        )
        .arg(
            Arg::with_name("quiet")
                .short("q")
                .long("quiet")
                .help("Don't print anything except standard output"),
        )
        .arg(
            Arg::with_name("URL")
                .index(1)
                .help("The url of the directory to crawl")
                .required(true)
                .takes_value(true),
        )
        .get_matches();

    let verbose = matches.occurrences_of("verbosity") as usize;
    let quiet = matches.is_present("quiet");
    stderrlog::new()
        .module(module_path!())
        .quiet(quiet)
        .verbosity(verbose)
        // .timestamp(ts)
        .init()
        .unwrap();

    if cfg!(unix) {
        unsafe {
            use libc::*;
            let mut r = rlimit {
                rlim_cur: 0,
                rlim_max: 0,
            };
            getrlimit(RLIMIT_NOFILE, &mut r);
            warn!(
                "increasing RLIMIT_NOFILE from {} to {}",
                r.rlim_cur, r.rlim_max
            );
            r.rlim_cur = r.rlim_max;
            setrlimit(RLIMIT_NOFILE, &r);
        }
    }

    use hyper::Uri;
    let uri = matches.value_of("URL").unwrap();
    let uri = uri.parse::<Uri>().unwrap_or_else(|e| {
        error!("Invalid url '{}': {}", uri, e);
        std::process::exit(1);
    });
    let mut parts = uri.into_parts();

    if parts.scheme == None {
        parts.scheme = Some(hyper::http::uri::Scheme::HTTP);
    }
    if parts.path_and_query == None {
        parts.path_and_query = Some(hyper::http::uri::PathAndQuery::from_static("/"));
    }

    let uri = Uri::from_parts(parts).unwrap();
    let current_url = Url::parse(&uri.to_string()).unwrap();
    let start = std::time::Instant::now();
    let mut rt = tokio::runtime::Runtime::new().unwrap();
    let fut = fetch::crawl(current_url);
    match rt.block_on(fut) {
        Err(e) => {
            error!("{}", e);
        }
        Ok(_) => {
            eprintln!("Finished in {}ms", start.elapsed().as_millis());
        }
    }
}

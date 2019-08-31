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

pub struct Settings {
    max_redirections: i64,
}

fn main() {
    let matches = App::new(env!("CARGO_PKG_NAME"))
        .max_term_width(80)
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
        .arg(
            Arg::with_name("max-redirections")
                .value_name("max")
                .long("max-redirections")
                .default_value("16")
                .help(
                    "The maximum number of times a resource may be redirected. \
                     Provide a negative number to allow infinite redirections",
                )
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

    let max_redirections = matches
        .value_of("max-redirections")
        .map(|v| {
            v.parse().unwrap_or_else(|_| {
                error!("max-redirections must be a valid 64-bit integer");
                std::process::exit(1);
            })
        })
        .unwrap();

    let settings = Box::leak(Box::new(Settings { max_redirections }));

    #[cfg(unix)]
    {
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
    let rt = tokio::runtime::Runtime::new().unwrap();
    let fut = fetch::crawl(current_url, settings);
    rt.block_on(fut);
    eprintln!("Finished in {}ms", start.elapsed().as_millis());
}

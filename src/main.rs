#![warn(bare_trait_objects)]

extern crate clap;
extern crate hyper;
extern crate robots_txt;
extern crate exit;
extern crate toks;
#[macro_use] extern crate html5ever;
extern crate url;
extern crate futures;

use clap::{App, Arg};
use hyper::Client;
use hyper::rt::{self, Future};
use exit::Exit;
use url::Url;

mod error;
mod tok;
mod fetch;
use error::CliError;

fn main() -> Exit<CliError> {
    let matches = App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .arg(Arg::with_name("URI")
            .index(1)
            .help("the uri of the directory to crawl")
            .required(true)
            .takes_value(true))
        .get_matches();

    let uri = matches.value_of("URI").unwrap();
    let current_url = Url::parse(uri)
        .map_err(|_| {
            CliError(format!("Uri has no authority: '{}'", uri))
        })?;
    let client = Client::new();
    let client = Box::leak(Box::new(client));
    let fut = fetch::traverse(client, current_url)
        .map_err(|e| eprintln!("{}", e))
        .map(|n| {
            println!("Result: {}", n);
        });
    rt::run(fut);
    println!("Done");
    Exit::Ok
}

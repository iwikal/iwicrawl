use crate::tok::SubdirTok;
use crate::error::CliError;
use futures::future::*;
use url::Url;
use hyper::{Client, rt::Stream};

fn extract_subdirs(body: hyper::Chunk, url: Url) -> SubdirTok {
    let s = std::str::from_utf8(&body)
        .unwrap();
    SubdirTok::from_body(url, s)
}

pub fn traverse(client: &'static Client<hyper::client::HttpConnector>, url: Url)
    -> Box<dyn Future<Item=u32, Error=CliError> + Send> {
    let f = loop_fn(url, move |url: Url| {
        use hyper::Uri;
        let uri: Uri = url.to_string().parse().unwrap();
        client
            .get(uri)
            .map_err(|e| {
                CliError(format!("{}", e))
            })
            .and_then(move |res| {
                println!("got     {}:\n{:#?}", url, res);
                let status = res.status();
                let headers = res.headers();

                if status.is_success() {
                    let f = res.into_body()
                        .concat2()
                        .map_err(|e| {
                            CliError(format!("{}", e))
                        })
                    .map(move |body| (body, url));
                    Ok(Loop::Break(f))
                } else if status.is_redirection() {
                    use hyper::header::*;
                    let s = headers[LOCATION]
                        .to_str()
                        .map_err(|e| {
                            CliError(format!("{}", e))
                        })?;
                    let url = Url::parse(s)
                        .map_err(|e| {
                            CliError(format!("{}", e))
                        })?;
                    Ok(Loop::Continue(url))
                } else {
                    Err(CliError(format!("Error: status {}", status)))
                }

            })
        })
        .flatten()
        .and_then(move |(body, url)| {
            let SubdirTok {
                paths,
                current_url,
            } = extract_subdirs(body, url);
            let subfutures = paths
                .into_iter()
                .map(move |path| {
                    let path_str = &path.to_string();
                    let url = current_url.join(path_str).unwrap();
                    traverse(client, url)
                });
            join_all(subfutures)
                .map(|subdirs| {
                    subdirs.iter()
                        .fold(0, |acc, cur| acc + cur)
                })
        });

    Box::new(f)
}


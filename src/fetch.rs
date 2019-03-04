use crate::tok::SubdirTok;
use crate::error::CliError;
use futures::future::*;
use url::Url;
use hyper::{Client, rt::Stream};

fn extract_subdirs(body: hyper::Chunk, url: Url) -> Result<SubdirTok, CliError> {
    let s = std::str::from_utf8(&body)
        .map_err(|e| CliError(format!("error parsing body: {}", e)))?;
    Ok(SubdirTok::from_body(url, s))
}

pub fn traverse(client: &'static Client<hyper::client::HttpConnector>, url: Url)
    -> Box<dyn Future<Item=u32, Error=CliError> + Send>
{
    let f = loop_fn(url, move |url: Url| {
        use hyper::Uri;
        let uri: Uri = url.to_string().parse().unwrap();
        client.get(uri)
            .map_err(|e| CliError(format!("{}", e)))
            .and_then(move |res| {
                debug!("{}:\n{:#?}", url, res);
                let status = res.status();
                let headers = res.headers();

                use hyper::header::*;
                if status.is_success() {
                    let content_type = headers
                        .get(CONTENT_TYPE)
                        .ok_or_else(|| CliError(format!("Content type missing")))?
                        .to_str()
                        .map_err(|e| CliError(format!("{}", e)))?;
                    if content_type.starts_with("text/html") {
                        let f = res.into_body()
                            .concat2()
                            .map_err(|e| CliError(format!("{}", e)))
                            .map(move |body| {
                                let subdirs = extract_subdirs(body, url).unwrap();
                                let SubdirTok {
                                    paths,
                                    current_url,
                                } = subdirs;
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
                        Ok(Loop::Break(Either::A(f.flatten())))
                    } else {
                        println!("{}", url);
                        Ok(Loop::Break(Either::B(futures::future::ok(1))))
                    }
                } else if status.is_redirection() {
                    let s = headers[LOCATION]
                        .to_str()
                        .map_err(|e| CliError(format!("{}", e)))?;
                    let new_url = Url::parse(s)
                        .map_err(|e| CliError(format!("{}", e)))?;
                    info!("{} redirected to {}: {}", url, new_url, status);
                    Ok(Loop::Continue(new_url))
                } else {
                    Err(CliError(format!("{} {}", status, url)))
                }
            })
    })
        .flatten();

    Box::new(f)
}


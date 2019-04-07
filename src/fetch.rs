use crate::error::{recursive_cause, CliError};
use crate::tok::SubdirTok;
use failure::{Error, Fail, ResultExt};
use futures::future::*;
use hyper::http::Method;
use hyper::{header::*, rt::Stream, Client, Request, Response};
use hyper_tls::HttpsConnector;
use url::Url;

fn extract_subdirs(body: hyper::Chunk, url: Url) -> Result<SubdirTok, Error> {
    let s = std::str::from_utf8(&body).map_err(|e| {
        e.context(format!("failed to parse body"))
            .context(format!("'{}'", url))
    })?;
    Ok(SubdirTok::from_body(url, s))
}

type MyClient = Client<HttpsConnector<hyper::client::HttpConnector>>;

const MAX_REDIRECTIONS: u8 = 16;
const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

fn follow_redirects(
    client: &'static MyClient,
    method: Method,
    url: Url,
) -> impl Future<Item = (Url, Response<hyper::Body>), Error = Error> {
    loop_fn((url, 0), move |(url, redirections_acc)| {
        use hyper::Uri;
        let uri: Uri = url.to_string().parse().unwrap();
        let request = Request::builder()
            .uri(uri.clone())
            .method(method.clone())
            .header("User-Agent", USER_AGENT)
            .body(Default::default())
            .unwrap();

        client
            .request(request)
            .map_err(|e| e.into())
            .and_then(move |res| {
                let status = res.status();

                if status.is_success() {
                    Ok(Loop::Break((url, res)))
                } else if status.is_redirection() {
                    if redirections_acc >= MAX_REDIRECTIONS {
                        Err(CliError(format!("too many redirections")).into())
                    } else {
                        let headers = res.headers();
                        let s = headers
                            .get(LOCATION)
                            .ok_or_else(|| CliError(format!("redirected to nowhere")))?
                            .to_str()?;
                        let new_url = url.join(s)?;
                        info!("{} redirected to {}: {}", url, new_url, status);
                        Ok(Loop::Continue((new_url, redirections_acc + 1)))
                    }
                } else {
                    Err(CliError(format!("{}", status)).into())
                }
            })
            .map_err(move |e: Error| e.context(format!("'{}'", uri)).into())
    })
}

type FutBox = Box<dyn Future<Item = u64, Error = Error> + Send>;

fn peek_file(client: &'static MyClient, url: Url) -> FutBox {
    let fut = follow_redirects(client, Method::HEAD, url);
    let fut = fut.and_then(|(redirected_url, res)| {
        let headers = res.headers();
        let bytes = headers
            .get(CONTENT_LENGTH)
            .ok_or_else(|| CliError(format!("content length missing")))
            .context(format!("'{}'", redirected_url))?
            .to_str()
            .context(format!("'{}'", redirected_url))?
            .parse::<u64>()
            .context(format!("'{}'", redirected_url))?;
        println!("{:<20} {}", bytes, redirected_url);
        Ok(bytes)
    });
    Box::new(fut)
}

fn handle_html_dir(
    client: &'static MyClient,
    body: hyper::Chunk,
    url: Url,
) -> Result<impl Future<Item = u64, Error = Error> + Send, Error> {
    let subdirs = extract_subdirs(body, url)?;
    let SubdirTok {
        paths, current_url, ..
    } = subdirs;
    let subfutures = {
        let current_url = current_url.clone();
        paths.into_iter().map(move |path| {
            let path_str = &path.to_string();
            let next_url = current_url.join(path_str).unwrap();
            if path_str.ends_with("/") {
                get_directory(client, next_url)
            } else {
                peek_file(client, next_url)
            }
            .or_else(|e| {
                error!("{}", recursive_cause(e.as_fail()));
                Ok(0)
            })
        })
    };
    let sum_children = join_all(subfutures).map(move |subdirs| {
        let sum = subdirs.iter().fold(0, |acc, cur| acc + cur);
        println!("{:<20} {}", sum, current_url);
        sum
    });
    Ok(sum_children)
}

fn get_directory(client: &'static MyClient, url: Url) -> FutBox {
    debug!("getting {}", url);
    let fut = follow_redirects(client, Method::GET, url)
        .and_then(move |(redirected_url, res)| {
            let headers = res.headers();
            let content_type = headers
                .get(CONTENT_TYPE)
                .ok_or_else(|| CliError(format!("'{}': content type missing", redirected_url)))
                .context(format!("'{}'", redirected_url))?
                .to_str()
                .context(format!("'{}'", redirected_url))?;
            debug!("content type {}", content_type);
            if content_type.starts_with("text/html") {
                let fut = res
                    .into_body()
                    .concat2()
                    .map_err(|e| e.into())
                    .and_then(move |body| handle_html_dir(client, body, redirected_url));
                Ok(fut)
            } else {
                // TODO: Other content types such as json
                Err(
                    CliError(format!("unexpected content type '{}'", content_type))
                        .context(format!("'{}'", redirected_url))
                        .into(),
                )
            }
        })
        .flatten()
        .flatten();

    Box::new(fut)
}

pub fn crawl(url: Url) -> FutBox {
    let connector = HttpsConnector::new(4).unwrap();
    let client = Client::builder().build::<_, hyper::Body>(connector);
    let client = Box::leak(Box::new(client));
    Box::new(get_directory(client, url).or_else(|e| {
        error!("{}", recursive_cause(e.as_fail()));
        Ok(0)
    }))
}

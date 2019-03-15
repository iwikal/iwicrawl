use crate::tok::SubdirTok;
use crate::error::CliError;
use futures::future::*;
use url::Url;
use hyper::{Request, Response, Client, rt::Stream, header::*};
use hyper::http::Method;

fn extract_subdirs(body: hyper::Chunk, url: Url) -> Result<SubdirTok, CliError> {
    let s = std::str::from_utf8(&body)
        .map_err(|e| CliError(format!("error parsing body: {}", e)))?;
    Ok(SubdirTok::from_body(url, s))
}

type MyClient = Client<hyper::client::HttpConnector>;

const MAX_REDIRECTIONS: u8 = 16;
fn follow_redirects(client: &'static MyClient, method: Method, url: Url)
-> impl Future<Item = (Url, Response<hyper::Body>), Error = CliError>
{
    loop_fn((url, 0), move |(url, redirections_acc)| {
        use hyper::Uri;
        let uri: Uri = url.to_string().parse().unwrap();
        let request = Request::builder()
            .uri(uri.clone())
            .method(method.clone())
            .body(Default::default())
            .unwrap();

        client.request(request)
            .map_err(move |e| CliError(format!("'{}': {}", uri, e)))
            .and_then(move |res| {
                let status = res.status();

                if status.is_success() {
                    Ok(Loop::Break((url, res)))
                } else if status.is_redirection() {
                    if redirections_acc >= MAX_REDIRECTIONS {
                        Err(CliError(format!("too many redirections")))
                    } else {
                        let headers = res.headers();
                        let s = headers[LOCATION]
                            .to_str()
                            .map_err(|e| CliError(format!("{}", e)))?;
                        let new_url = Url::parse(s)
                            .map_err(|e| CliError(format!("{}", e)))?;
                        info!("{} redirected to {}: {}", url, new_url, status);
                        Ok(Loop::Continue((new_url, redirections_acc + 1)))
                    }
                } else {
                    Err(CliError(format!("{} {}", status, url)))
                }
            })
    })
}

fn peek_file(client: &'static MyClient, url: Url)
    -> Box<dyn Future<Item=u64, Error=CliError> + Send>
{
    let fut = follow_redirects(client, Method::HEAD, url);
    let fut = fut
        .map(|(redirected_url, res)| {
            let headers = res.headers();
            let bytes = headers[CONTENT_LENGTH]
                .to_str()
                .map_err(|e| {
                    CliError(format!("can't parse content length header: {}", e))
                })?
            .parse::<u64>()
                .map_err(|e| {
                    CliError(format!("can't parse content length header: {}", e))
                })?;
            println!("{:<8} {}", bytes, redirected_url);
            Ok(bytes)
        });
    Box::new(fut.flatten())
}

pub fn get_directory(client: &'static MyClient, url: Url)
    -> Box<dyn Future<Item=u64, Error=CliError> + Send>
{
    info!("getting {}", url);
    let fut = follow_redirects(client, Method::GET, url)
        .map(move |(redirected_url, res)| {
            let headers = res.headers();
            let content_type = headers
                .get(CONTENT_TYPE)
                .ok_or_else(|| CliError(format!("content type missing")))?
                .to_str()
                .map_err(|e| CliError(format!("{}", e)))?;
            debug!("content type {}", content_type);
            if content_type.starts_with("text/html") {
                let fut = res.into_body()
                    .concat2()
                    .map_err(|e| CliError(format!("{}", e)))
                    .and_then(move |body| {
                        let subdirs = extract_subdirs(body, redirected_url)
                            .map_err(|e| CliError(format!("failed to parse body: {}", e)))?;
                        let SubdirTok {
                            paths,
                            current_url,
                        } = subdirs;
                        let subfutures = {
                            let current_url = current_url.clone();
                            paths
                                .into_iter()
                                .map(move |path| {
                                    let path_str = &path.to_string();
                                    let next_url = current_url.join(path_str).unwrap();
                                    if path_str.ends_with("/") {
                                        get_directory(client, next_url)
                                    } else {
                                        peek_file(client, next_url)
                                    }
                                })
                        };
                        let sum_children = join_all(subfutures)
                            .map(move |subdirs| {
                                let sum = subdirs.iter()
                                    .fold(0, |acc, cur| acc + cur);
                                println!("{:<8} {}", sum, current_url);
                                sum
                            });
                        Ok(sum_children)
                    });
                Ok(fut)
            } else {
                // TODO: Other content types such as json
                Err(CliError(format!("{}: unrecognised content type '{}'", redirected_url, content_type)))
            }
        })
    .flatten()
    .flatten()
    .flatten();

    Box::new(fut)
}

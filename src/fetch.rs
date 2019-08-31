use crate::error::{recursive_cause, Error};
use crate::tok::SubdirTok;
use crate::Settings;
use encoding::label::encoding_from_whatwg_label;
use encoding::types::DecoderTrap;
use failure::{Fail, ResultExt};
use futures_util::{future::join_all, TryStreamExt};
use hyper::http::Method;
use hyper::{header::*, Client, Request, Response};
use hyper_tls::HttpsConnector;
use url::Url;

struct Context {
    client: Client<HttpsConnector<hyper::client::HttpConnector>>,
    settings: Settings,
}

const USER_AGENT: &str =
    concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

async fn follow_redirects(
    context: &Context,
    method: Method,
    url: Url,
) -> Result<(Url, Response<hyper::Body>), failure::Error> {
    let Context { client, settings } = context;
    let mut url = url;
    let mut redirections = 0;
    loop {
        use hyper::Uri;
        let uri: Uri = url.to_string().parse().unwrap();
        let request = Request::builder()
            .uri(uri.clone())
            .method(method.clone())
            .header("User-Agent", USER_AGENT)
            .body(Default::default())
            .unwrap();

        let res = client.request(request).await?;
        let status = res.status();

        if status.is_success() {
            return Ok((url, res));
        } else if status.is_redirection() {
            redirections += 1;
            let max = settings.max_redirections;
            if max >= 0 && redirections >= max {
                return Err(Error::new(format!(
                    "too many redirections: {}",
                    redirections as u64
                ))
                .into());
            } else {
                let headers = res.headers();
                let s = headers
                    .get(LOCATION)
                    .ok_or_else(|| {
                        Error::new(format!("redirected to nowhere"))
                    })?
                    .to_str()?;
                let new_url = url.join(s)?;
                info!("{} redirected to {}: {}", url, new_url, status);
                url = new_url;
                continue;
            }
        } else {
            return Err(Error::new(format!("{}", status)).into());
        }
    }
}

async fn peek_file(context: &Context, url: Url) -> Result<u64, failure::Error> {
    let (redirected_url, res) =
        follow_redirects(context, Method::HEAD, url).await?;
    let headers = res.headers();
    let bytes = headers
        .get(CONTENT_LENGTH)
        .ok_or_else(|| {
            Error::new(format!(
                "content length missing, status {}",
                res.status()
            ))
        })
        .context(format!("'{}'", redirected_url))?
        .to_str()
        .context(format!("'{}'", redirected_url))?
        .parse::<u64>()
        .context(format!("'{}'", redirected_url))?;
    println!("{:<20} {}", bytes, redirected_url);
    Ok(bytes)
}

fn report_and_default_to_zero(e: failure::Error) -> u64 {
    error!("{}", recursive_cause(e.as_fail()));
    0
}

async fn handle_html_dir(context: &Context, body: &str, url: Url) -> u64 {
    let SubdirTok {
        paths, current_url, ..
    } = SubdirTok::from_body(url, body);
    let sum = join_all(paths.into_iter().map(|path| {
        let url = current_url.join(&path.to_string()).unwrap();
        async move {
            if url.path().ends_with("/") {
                get_directory(context, url).await
            } else {
                peek_file(context, url).await
            }
            .unwrap_or_else(report_and_default_to_zero)
        }
    })).await.iter().fold(0, |acc, cur| acc + cur);
    println!("{:<20} {}", sum, current_url);
    sum
}

async fn get_directory(
    context: &Context,
    url: Url,
) -> Result<u64, failure::Error> {
    debug!("getting {}", url);
    let (redirected_url, res) =
        follow_redirects(context, Method::GET, url).await?;
    let headers = res.headers();
    let content_type = headers
        .get(CONTENT_TYPE)
        .ok_or_else(|| {
            Error::new(format!("'{}': content type missing", redirected_url))
        })
        .context(format!("'{}'", redirected_url))?
        .to_str()
        .context(format!("'{}'", redirected_url))?;
    debug!("content type {}", content_type);
    use mime::*;
    let mime = content_type
        .parse::<Mime>()
        .context(format!("'{}'", redirected_url))?;
    if (mime.type_(), mime.subtype()) == (TEXT, HTML) {
        let body = res.into_body().try_concat().await?;
        let charset = mime.get_param(CHARSET).unwrap_or(UTF_8);
        let encoding = encoding_from_whatwg_label(charset.into()).unwrap();
        let body = encoding
            .decode(&body, DecoderTrap::Replace)
            .map_err(|e| Error::new(e.to_string()))
            .context(format!("'{}'", redirected_url))?;
        Ok(handle_html_dir(context, &body, redirected_url).await)
    } else {
        // TODO: Other content types such as json
        Err(
            Error::new(format!("unexpected content type '{}'", content_type))
                .context(format!("'{}'", redirected_url))
                .into(),
        )
    }
}

pub async fn crawl(url: Url, settings: Settings) -> u64 {
    let connector = HttpsConnector::new().unwrap();
    let client = Client::builder().build::<_, hyper::Body>(connector);
    let context = Context { client, settings };

    get_directory(&context, url)
        .await
        .unwrap_or_else(report_and_default_to_zero)
}

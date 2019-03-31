use html5ever::parse_document;
use html5ever::rcdom::{Handle, RcDom};
use html5ever::tendril::{self, NonAtomic, Tendril, TendrilSink};
use std::collections::HashSet;
use toks::prelude::*;
use toks::{recursion, Tok};
use url::Url;

#[derive(Debug)]
pub struct SubdirTok {
    pub current_url: Url,
    pub paths: Vec<Tendril<tendril::fmt::UTF8, NonAtomic>>,
    unique_set: HashSet<Url>,
}

impl SubdirTok {
    pub fn new(current_url: Url) -> Self {
        Self {
            current_url,
            paths: Default::default(),
            unique_set: Default::default(),
        }
    }

    pub fn from_body(url: Url, body: &str) -> Self {
        let dom = parse_document(RcDom::default(), Default::default()).one(body);
        let mut subdir_tok = SubdirTok::new(url);
        recursion(&mut vec![&mut subdir_tok], dom.document);
        subdir_tok
    }
}

fn normalize_path(segments: core::str::Split<char>) -> Option<Vec<&str>> {
    { segments }.try_fold(Vec::new(), |mut acc, cur| match cur {
        "" | "." => Some(acc),
        ".." => {
            acc.pop()?;
            Some(acc)
        }
        _ => {
            acc.push(cur);
            Some(acc)
        }
    })
}

impl Tok for SubdirTok {
    fn is_match(&self, qn: &QualName) -> bool {
        qn.local == local_name!("a")
    }

    fn process(&mut self, attribs: RefCell<Vec<Attribute>>, _: RefCell<Vec<Handle>>) {
        let SubdirTok {
            paths,
            current_url,
            unique_set,
        } = self;

        let current_path =
            normalize_path(current_url.path_segments().unwrap()).expect("invalid path");

        attribs
            .into_inner()
            .into_iter()
            .filter(|attrib| attrib.name.local == local_name!("href"))
            .for_each(|href| {
                let s = &href.value.to_string();
                // TODO: more efficient url parsing
                let url = match current_url.join(s) {
                    Ok(mut url) => {
                        url.set_query(None);
                        url.set_fragment(None);
                        url
                    }
                    Err(e) => {
                        eprintln!("Error parsing '{}': {}", s, e);
                        return;
                    }
                };
                if unique_set.contains(&url) {
                    debug!("duplicate url {}", url);
                    return;
                }

                if url.scheme() != current_url.scheme()
                    || url.host_str() != current_url.host_str()
                    || url.port_or_known_default().unwrap()
                        != current_url.port_or_known_default().unwrap()
                {
                    return;
                }

                let mut segments = match url.path_segments() {
                    Some(x) => x,
                    None => return,
                };
                match segments.try_fold(0_u32, |acc, cur| match cur {
                    "" | "." => Some(acc),
                    ".." => acc.checked_sub(1),
                    _ => {
                        if let Some(seg) = current_path.get(acc as usize) {
                            if seg != &cur {
                                return None;
                            }
                        }
                        Some(acc + 1)
                    }
                }) {
                    Some(depth) if depth > current_path.len() as u32 => {}
                    _ => {
                        return;
                    }
                }

                unique_set.insert(url);
                paths.push(href.value);
            });
    }
}

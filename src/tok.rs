use html5ever::parse_document;
use html5ever::rcdom::{Handle, RcDom};
use html5ever::tendril::TendrilSink;
use std::collections::HashSet;
use toks::prelude::*;
use toks::{recursion, Tok};
use url::Url;

#[derive(Debug)]
pub struct SubdirTok {
    pub current_url: Url,
    pub subdirectories: HashSet<Url>,
    pub files: HashSet<Url>,
}

impl SubdirTok {
    pub fn new(current_url: Url) -> Self {
        Self {
            current_url,
            subdirectories: Default::default(),
            files: Default::default(),
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
            current_url,
            subdirectories,
            files,
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
                        error!("Can't parse href=\"{}\": {}", s, e);
                        return;
                    }
                };

                if url.path().ends_with("/") {
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
                    if !subdirectories.insert(url) {
                        debug!("duplicate link");
                    }
                } else {
                    if !files.insert(url) {
                        debug!("duplicate link");
                    }
                }
            });
    }
}

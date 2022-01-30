use lazy_static::lazy_static;
use log::debug;
use regex::Regex;

#[derive(Debug, Default, Clone)] //TODO: remove clone
pub struct RefNames {
    pub head: Option<String>,
    pub tags: Vec<String>,
    pub heads: Vec<String>,
    pub remotes: Vec<String>,
}

impl RefNames {
    pub fn from(ref_specs: &str) -> Option<Self> {
        if ref_specs.is_empty() {
            return None;
        }
        lazy_static! {
            static ref RE: Regex = Regex::new(
                r#"(?x)
                (?P<is_head>HEAD(?:\ ->\ )?)?
                (?:
                    (tag:\ refs/tags/)(?P<tag>.+) |
                    (refs/heads/)(?P<head>.+) |
                    (refs/remotes/)(?P<remote>.+)
                )?
                "#,
            )
            .unwrap();
        }
        let refs = ref_specs
            .split(", ")
            .fold(RefNames::default(), |mut acc, ref_spec| {
                debug!("ref_spect='{}'", ref_spec);
                let c = RE.captures(ref_spec).unwrap();
                if c.name("is_head").is_some() {
                    assert!(acc.head.is_none(), "Cannot have two heads!");
                    // knowing that 'HEAD -> ' is always first (see regex)
                    // we can take element after it as name of current
                    acc.head = Some(
                        c.name("head")
                            .or_else(|| c.name("tag"))
                            .or_else(|| c.name("remote"))
                            .map_or_else(|| "HEAD", |f| f.as_str())
                            .to_string(),
                    );
                } else if let Some(head) = c.name("head") {
                    acc.heads.push(head.as_str().to_string())
                } else if let Some(tag) = c.name("tag") {
                    acc.tags.push(tag.as_str().to_string())
                } else if let Some(remote) = c.name("remote") {
                    acc.remotes.push(remote.as_str().to_string())
                }

                acc
            });
        Some(refs)
    }
}

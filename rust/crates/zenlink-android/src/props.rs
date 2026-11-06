//! `getprop` output: `[key]: [value]` per line. Values can contain `]` and,
//! rarely, newlines; old devices end lines with `\r\r\n`.

use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Props(BTreeMap<String, String>);

impl Props {
    /// Parses bytes as ADB delivers them (invalid UTF-8 is replaced, not rejected).
    pub fn parse_bytes(raw: &[u8]) -> Self {
        Self::parse(&String::from_utf8_lossy(raw))
    }

    pub fn parse(text: &str) -> Self {
        let text = text.replace("\r\n", "\n").replace('\r', "");
        let mut map = BTreeMap::new();
        let mut pending: Option<(String, String)> = None;
        for line in text.split('\n') {
            if let Some((key, mut value)) = pending.take() {
                // Continuation of a multi-line value.
                if let Some(v) = line.strip_suffix(']') {
                    value.push('\n');
                    value.push_str(v);
                    map.insert(key, value);
                } else {
                    value.push('\n');
                    value.push_str(line);
                    pending = Some((key, value));
                }
                continue;
            }
            let Some(rest) = line.strip_prefix('[') else {
                continue;
            };
            let Some((key, value)) = rest.split_once("]: [") else {
                continue;
            };
            if key.is_empty() {
                continue;
            }
            match value.strip_suffix(']') {
                Some(v) => {
                    map.insert(key.to_string(), v.to_string());
                }
                None => pending = Some((key.to_string(), value.to_string())),
            }
        }
        Self(map)
    }

    /// The raw value, or `None` when missing.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    /// The first key that has a non-empty value after trimming.
    pub fn first(&self, keys: &[&str]) -> Option<&str> {
        keys.iter()
            .filter_map(|k| self.get(k))
            .map(str::trim)
            .find(|v| !v.is_empty())
    }

    pub fn from_pairs<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        Self(
            pairs
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        )
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_getprop() {
        let p = Props::parse(
            "[ro.product.model]: [SM-A576B]\r\r\n[ro.product.manufacturer]: [samsung]\r\r\n\
             [weird]: [a]: [b]]\n[empty]: []\n[multi]: [line one\nline two]\n[after]: [ok]\nnot a prop\n[]: [x]\n",
        );
        assert_eq!(p.get("ro.product.model"), Some("SM-A576B"));
        assert_eq!(p.get("ro.product.manufacturer"), Some("samsung"));
        assert_eq!(p.get("weird"), Some("a]: [b]"));
        assert_eq!(p.get("empty"), Some(""));
        assert_eq!(p.get("multi"), Some("line one\nline two"));
        assert_eq!(p.get("after"), Some("ok"));
        assert_eq!(p.len(), 6);
    }

    #[test]
    fn invalid_utf8_is_kept_as_replacement() {
        let p = Props::parse_bytes(b"[ro.product.marketname]: [Phone \xff\xfe X]\n");
        assert_eq!(
            p.get("ro.product.marketname"),
            Some("Phone \u{fffd}\u{fffd} X")
        );
    }

    #[test]
    fn first_skips_blank() {
        let p = Props::from_pairs([("a", "  "), ("b", " x ")]);
        assert_eq!(p.first(&["missing", "a", "b"]), Some("x"));
        assert_eq!(p.first(&["a"]), None);
    }
}

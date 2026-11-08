//! Human-readable device names (master prompt section 4, "Real device names").
//!
//! Policy:
//! - Names come only from properties of an authorized device. Nothing is
//!   inferred from model codes and there is no model-name database.
//! - A reported marketing name wins. Without one, the platform model name is
//!   shown with the brand in front; when that is only a code (`SM-A576B`) the
//!   result is the truthful "Samsung SM-A576B", not an invented product name.
//! - Brand prefixes are never doubled ("Samsung Samsung ...").
//! - Unauthorized or offline devices get a placeholder, never a guessed name.
//! - A user nickname is kept apart from the detected name (see [`Label`]).

use crate::props::Props;

/// Marketing-name properties, most specific first.
///
/// `ro.product.marketname` is used by several vendors (e.g. TECNO, Xiaomi,
/// realme). The vendor-specific keys below are read only when present; their
/// availability across vendors and Android versions is not verified on
/// hardware yet (ZenshiroDev/docs/android/NAMING.md).
pub const MARKETING_PROPS: &[&str] = &[
    "ro.product.marketname",
    "ro.product.vendor.marketname",
    "ro.product.odm.marketname",
    "ro.vendor.oplus.market.name",
    "ro.oppo.market.name",
    "ro.vivo.market.name",
    "ro.config.marketing_name",
];

/// Properties read for a device after authorization (all read-only).
pub const NAME_PROPS: &[&str] = &[
    "ro.product.manufacturer",
    "ro.product.brand",
    "ro.product.model",
    "ro.product.name",
    "ro.product.device",
    "ro.build.version.release",
    "ro.build.version.sdk",
    "ro.build.characteristics",
    "ro.serialno",
    "ro.boot.serialno",
];

/// Canonical spelling of brands, keyed by lowercase. Brands not listed keep
/// their reported spelling (or get a capital first letter if all lowercase).
const BRANDS: &[&str] = &[
    "Samsung",
    "Xiaomi",
    "Redmi",
    "POCO",
    "OPPO",
    "OnePlus",
    "vivo",
    "iQOO",
    "realme",
    "Google",
    "Motorola",
    "ASUS",
    "Lenovo",
    "TECNO",
    "Infinix",
    "itel",
    "HONOR",
    "HUAWEI",
    "Nothing",
    "Sony",
    "Nokia",
    "ZTE",
    "nubia",
    "Meizu",
    "LG",
    "HTC",
    "Sharp",
    "Fairphone",
    "Blackview",
    "Ulefone",
    "Doogee",
    "Cubot",
    "Oukitel",
    "Umidigi",
];

/// Values vendors leave in properties that mean "not set".
const PLACEHOLDERS: &[&str] = &[
    "unknown",
    "null",
    "none",
    "n/a",
    "na",
    "default",
    "undefined",
    "not specified",
    "to be filled by o.e.m.",
    "generic",
];

/// Where the shown name came from (shown as "name source" in device details).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NameSource {
    /// A marketing-name property; holds the property key.
    Marketing(&'static str),
    /// `ro.product.model` that reads like a product name ("Pixel 8").
    Model,
    /// `ro.product.model` that is only a model code ("SM-A576B").
    ModelCode,
    /// The `model:` token of `adb devices -l` (properties were unreadable).
    AdbList,
    /// No usable name: unauthorized, offline, or nothing reported.
    Placeholder,
}

impl NameSource {
    /// Rough confidence for the details view.
    pub fn confidence(&self) -> &'static str {
        match self {
            Self::Marketing(_) => "high",
            Self::Model => "medium",
            Self::ModelCode | Self::AdbList => "low",
            Self::Placeholder => "none",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceName {
    pub display: String,
    pub source: NameSource,
}

/// Removes control and invisible characters, collapses whitespace, and
/// returns `None` for empty or placeholder values.
pub fn clean(raw: &str) -> Option<String> {
    let mapped: String = raw
        .chars()
        .map(|c| match c {
            '\u{200B}'..='\u{200D}' | '\u{2060}' | '\u{FEFF}' | '\u{FFFD}' => '\0',
            c if c.is_control() => ' ',
            c => c,
        })
        .filter(|&c| c != '\0')
        .collect();
    let out = mapped.split_whitespace().collect::<Vec<_>>().join(" ");
    let out = out
        .trim_matches(|c| c == '"' || c == '\'')
        .trim()
        .to_string();
    if out.is_empty() || PLACEHOLDERS.contains(&out.to_lowercase().as_str()) {
        None
    } else {
        Some(out)
    }
}

/// Canonical brand spelling.
pub fn brand_label(raw: &str) -> String {
    let lower = raw.to_lowercase();
    if let Some(b) = BRANDS.iter().find(|b| b.to_lowercase() == lower) {
        return b.to_string();
    }
    if raw.chars().all(|c| !c.is_uppercase()) {
        let mut cs = raw.chars();
        match cs.next() {
            Some(f) => f.to_uppercase().chain(cs).collect(),
            None => String::new(),
        }
    } else {
        raw.to_string()
    }
}

/// If `name` starts with a known brand (or `brand`) as a whole word, returns
/// the name with that prefix in canonical spelling and any doubled prefix removed.
fn normalize_prefix(name: &str, brand: Option<&str>) -> Option<String> {
    let mut words: Vec<&str> = name.split(' ').collect();
    let first = words.first()?.to_lowercase();
    let known = BRANDS.iter().any(|b| b.to_lowercase() == first)
        || brand.is_some_and(|b| b.to_lowercase() == first);
    if !known {
        return None;
    }
    while words.len() > 1 && words[1].to_lowercase() == first {
        words.remove(1);
    }
    let label = brand_label(words[0]);
    Some(
        std::iter::once(label.as_str())
            .chain(words[1..].iter().copied())
            .collect::<Vec<_>>()
            .join(" "),
    )
}

/// Puts the brand in front unless the name already starts with a brand.
fn with_brand(name: &str, brand: Option<&str>) -> String {
    if let Some(n) = normalize_prefix(name, brand) {
        return n;
    }
    match brand {
        Some(b) => format!("{} {name}", brand_label(b)),
        None => name.to_string(),
    }
}

/// A model string that is an identifier, not a product name: one word with a
/// digit, e.g. `SM-A576B`, `23117RA68G`, `CPH2591`, `LH8n`.
pub fn looks_like_code(model: &str) -> bool {
    !model.contains(' ') && model.chars().any(|c| c.is_ascii_digit())
}

/// The brand to show: `ro.product.brand` (it carries sub-brands such as
/// Redmi or POCO), else `ro.product.manufacturer`.
pub fn brand(props: &Props) -> Option<String> {
    ["ro.product.brand", "ro.product.manufacturer"]
        .iter()
        .filter_map(|k| props.get(k))
        .find_map(clean)
}

/// Resolves the name of an authorized device from its properties.
pub fn resolve(props: &Props) -> DeviceName {
    let brand = brand(props);
    let b = brand.as_deref();
    for key in MARKETING_PROPS {
        if let Some(m) = props.get(key).and_then(clean) {
            return DeviceName {
                display: with_brand(&m, b),
                source: NameSource::Marketing(key),
            };
        }
    }
    if let Some(model) = props.get("ro.product.model").and_then(clean) {
        let source = if looks_like_code(&model) {
            NameSource::ModelCode
        } else {
            NameSource::Model
        };
        return DeviceName {
            display: with_brand(&model, b),
            source,
        };
    }
    match brand {
        Some(b) => DeviceName {
            display: format!("{} Android device", brand_label(&b)),
            source: NameSource::Placeholder,
        },
        None => placeholder(None),
    }
}

/// Name from the `model:` token of `adb devices -l`. ADB replaces spaces and
/// other characters (also `-`) with `_`, so the original cannot be restored;
/// the token is shown as reported until the properties are readable.
pub fn from_adb_model(token: &str) -> Option<DeviceName> {
    let model = clean(token)?;
    Some(DeviceName {
        display: with_brand(&model, None),
        source: NameSource::AdbList,
    })
}

/// A truthful placeholder for a device whose properties are not readable.
pub fn placeholder(serial: Option<&str>) -> DeviceName {
    let display = match serial.and_then(clean) {
        Some(s) => format!("Android device ({s})"),
        None => "Android device".to_string(),
    };
    DeviceName {
        display,
        source: NameSource::Placeholder,
    }
}

/// What the UI shows: the user's nickname first, the detected name below it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub title: String,
    pub subtitle: Option<String>,
}

pub fn label(nickname: Option<&str>, detected: &DeviceName) -> Label {
    match nickname.and_then(clean) {
        Some(n) if n != detected.display => Label {
            title: n,
            subtitle: Some(detected.display.clone()),
        },
        _ => Label {
            title: detected.display.clone(),
            subtitle: None,
        },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormFactor {
    Phone,
    Tablet,
    Tv,
    Watch,
    Automotive,
    Unknown,
}

/// From `ro.build.characteristics` (comma-separated, e.g. `tablet,nosdcard`).
pub fn form_factor(props: &Props) -> FormFactor {
    let Some(c) = props.get("ro.build.characteristics").and_then(clean) else {
        return FormFactor::Unknown;
    };
    let tags: Vec<String> = c.split(',').map(|t| t.trim().to_lowercase()).collect();
    let has = |t: &str| tags.iter().any(|x| x == t);
    if has("tablet") {
        FormFactor::Tablet
    } else if has("tv") {
        FormFactor::Tv
    } else if has("watch") {
        FormFactor::Watch
    } else if has("automotive") {
        FormFactor::Automotive
    } else {
        FormFactor::Phone
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(pairs: &[(&str, &str)]) -> Props {
        Props::from_pairs(pairs.iter().copied())
    }

    // Fixture values below are synthetic test inputs, not detected devices.

    #[test]
    fn samsung_model_code_is_shown_truthfully() {
        let n = resolve(&p(&[
            ("ro.product.manufacturer", "samsung"),
            ("ro.product.brand", "samsung"),
            ("ro.product.model", "SM-A576B"),
        ]));
        assert_eq!(n.display, "Samsung SM-A576B");
        assert_eq!(n.source, NameSource::ModelCode);
    }

    #[test]
    fn samsung_marketing_name_gets_brand() {
        let n = resolve(&p(&[
            ("ro.product.brand", "samsung"),
            ("ro.product.model", "SM-A576B"),
            ("ro.product.marketname", "Galaxy A57 5G"),
        ]));
        assert_eq!(n.display, "Samsung Galaxy A57 5G");
        assert_eq!(n.source, NameSource::Marketing("ro.product.marketname"));
    }

    #[test]
    fn tecno_marketing_name_not_board_id() {
        let n = resolve(&p(&[
            ("ro.product.manufacturer", "TECNO"),
            ("ro.product.brand", "TECNO"),
            ("ro.product.model", "TECNO LH8n"),
            ("ro.product.device", "TECNO-LH8n"),
            ("ro.product.marketname", "TECNO POVA 6"),
        ]));
        assert_eq!(n.display, "TECNO POVA 6");
    }

    #[test]
    fn tecno_without_marketname_falls_back_to_model() {
        let n = resolve(&p(&[
            ("ro.product.brand", "TECNO"),
            ("ro.product.model", "TECNO LH8n"),
        ]));
        assert_eq!(n.display, "TECNO LH8n");
        assert_eq!(n.source, NameSource::Model);
    }

    #[test]
    fn xiaomi_sub_brands() {
        let redmi = resolve(&p(&[
            ("ro.product.manufacturer", "Xiaomi"),
            ("ro.product.brand", "Redmi"),
            ("ro.product.model", "23117RA68G"),
            ("ro.product.marketname", "Redmi Note 13 Pro 5G"),
        ]));
        assert_eq!(redmi.display, "Redmi Note 13 Pro 5G");
        let poco = resolve(&p(&[
            ("ro.product.brand", "POCO"),
            ("ro.product.marketname", "poco X6 Pro 5G"),
        ]));
        assert_eq!(poco.display, "POCO X6 Pro 5G");
        let xiaomi_brand_redmi_name = resolve(&p(&[
            ("ro.product.brand", "Xiaomi"),
            ("ro.product.marketname", "Redmi 13C"),
        ]));
        assert_eq!(
            xiaomi_brand_redmi_name.display, "Redmi 13C",
            "no 'Xiaomi Redmi' doubling"
        );
        let code = resolve(&p(&[
            ("ro.product.brand", "Redmi"),
            ("ro.product.model", "23108RN04Y"),
        ]));
        assert_eq!(code.display, "Redmi 23108RN04Y");
        assert_eq!(code.source, NameSource::ModelCode);
    }

    #[test]
    fn pixel_and_lowercase_brands() {
        let n = resolve(&p(&[
            ("ro.product.brand", "google"),
            ("ro.product.model", "Pixel 8"),
        ]));
        assert_eq!(
            (n.display.as_str(), &n.source),
            ("Google Pixel 8", &NameSource::Model)
        );
        let v = resolve(&p(&[
            ("ro.product.brand", "VIVO"),
            ("ro.product.model", "V2318"),
        ]));
        assert_eq!(v.display, "vivo V2318");
        let r = resolve(&p(&[
            ("ro.product.brand", "realme"),
            ("ro.product.model", "RMX3890"),
        ]));
        assert_eq!(r.display, "realme RMX3890");
        let o = resolve(&p(&[
            ("ro.product.brand", "oppo"),
            ("ro.vendor.oplus.market.name", "OPPO Reno11 F 5G"),
        ]));
        assert_eq!(o.display, "OPPO Reno11 F 5G");
        let unknown = resolve(&p(&[
            ("ro.product.brand", "acmephone"),
            ("ro.product.model", "X1"),
        ]));
        assert_eq!(unknown.display, "Acmephone X1");
    }

    #[test]
    fn duplicate_brand_prefix_is_removed() {
        let n = resolve(&p(&[
            ("ro.product.brand", "samsung"),
            ("ro.product.marketname", "Samsung Samsung Galaxy Tab S9"),
        ]));
        assert_eq!(n.display, "Samsung Galaxy Tab S9");
        let m = resolve(&p(&[
            ("ro.product.brand", "Infinix"),
            ("ro.product.model", "infinix X6871"),
        ]));
        assert_eq!(m.display, "Infinix X6871");
    }

    #[test]
    fn placeholders_are_ignored() {
        let n = resolve(&p(&[
            ("ro.product.brand", "unknown"),
            ("ro.product.manufacturer", "motorola"),
            ("ro.product.marketname", "  "),
            ("ro.product.vendor.marketname", "null"),
            ("ro.product.model", "moto g54 5G"),
        ]));
        assert_eq!(n.display, "Motorola moto g54 5G");
        let empty = resolve(&Props::default());
        assert_eq!(
            (empty.display.as_str(), &empty.source),
            ("Android device", &NameSource::Placeholder)
        );
        let brand_only = resolve(&p(&[("ro.product.brand", "nokia")]));
        assert_eq!(brand_only.display, "Nokia Android device");
        assert_eq!(brand_only.source, NameSource::Placeholder);
    }

    #[test]
    fn malformed_unicode_and_whitespace() {
        assert_eq!(
            clean("  Galaxy\u{200B} A57\t\n 5G \u{FEFF}").as_deref(),
            Some("Galaxy A57 5G")
        );
        assert_eq!(clean("Phone\u{FFFD}\u{FFFD} X").as_deref(), Some("Phone X"));
        assert_eq!(clean("\"Pixel 8\"").as_deref(), Some("Pixel 8"));
        assert_eq!(clean("\u{0}\u{7}"), None);
        let n = resolve(&p(&[
            ("ro.product.brand", " Google\r"),
            ("ro.product.model", "Pixel\u{00A0}8a "),
        ]));
        assert_eq!(n.display, "Google Pixel 8a");
    }

    #[test]
    fn tablet_form_factor() {
        let props = p(&[
            ("ro.product.brand", "samsung"),
            ("ro.product.model", "SM-X710"),
            ("ro.build.characteristics", "tablet,nosdcard"),
        ]);
        assert_eq!(form_factor(&props), FormFactor::Tablet);
        assert_eq!(resolve(&props).display, "Samsung SM-X710");
        assert_eq!(
            form_factor(&p(&[("ro.build.characteristics", "nosdcard")])),
            FormFactor::Phone
        );
        assert_eq!(form_factor(&Props::default()), FormFactor::Unknown);
    }

    #[test]
    fn unauthorized_placeholder_and_adb_token() {
        let ph = placeholder(Some("0B261FDF6000AB"));
        assert_eq!(ph.display, "Android device (0B261FDF6000AB)");
        assert_eq!(ph.source.confidence(), "none");
        let a = from_adb_model("Pixel_8").unwrap();
        assert_eq!(a.display, "Pixel_8");
        assert_eq!(a.source, NameSource::AdbList);
        assert!(from_adb_model("").is_none());
    }

    #[test]
    fn nickname_first_detected_below() {
        let d = DeviceName {
            display: "Google Pixel 8".into(),
            source: NameSource::Model,
        };
        assert_eq!(
            label(Some("Work phone"), &d),
            Label {
                title: "Work phone".into(),
                subtitle: Some("Google Pixel 8".into())
            }
        );
        assert_eq!(
            label(Some("  "), &d),
            Label {
                title: "Google Pixel 8".into(),
                subtitle: None
            }
        );
        assert_eq!(label(None, &d).title, "Google Pixel 8");
    }
}

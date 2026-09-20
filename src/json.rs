//! A thin layer over serde_json, so the rest of omatide reads and writes
//! JSON the same way whether it came from NOAA or from our own cache.

pub use serde_json::Value as Json;

pub fn parse(text: &str) -> Result<Json, String> {
    serde_json::from_str(text).map_err(|e| format!("bad JSON: {e}"))
}

/// A string as a JSON literal, quoted and escaped.
pub fn string(s: &str) -> String {
    Json::String(s.to_string()).to_string()
}

/// A number NOAA may have sent as a number, as a string, or as null.
pub fn number(v: Option<&Json>) -> Option<f64> {
    match v? {
        Json::Number(n) => n.as_f64(),
        Json::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// Text NOAA may have sent as a string or a bare number.
pub fn text(v: Option<&Json>) -> Option<String> {
    match v? {
        Json::String(s) => Some(s.clone()),
        Json::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_numbers_however_noaa_sent_them() {
        let v = parse(r#"{"a": 1.5, "b": "2.5", "c": null, "d": " 3 ", "e": "x"}"#).unwrap();
        assert_eq!(number(v.get("a")), Some(1.5));
        assert_eq!(number(v.get("b")), Some(2.5));
        assert_eq!(number(v.get("c")), None);
        assert_eq!(number(v.get("d")), Some(3.0));
        assert_eq!(number(v.get("e")), None);
        assert_eq!(number(v.get("missing")), None);
    }

    #[test]
    fn reads_text_however_noaa_sent_it() {
        let v = parse(r#"{"a": "PCT0291", "b": 9, "c": null}"#).unwrap();
        assert_eq!(text(v.get("a")).as_deref(), Some("PCT0291"));
        assert_eq!(text(v.get("b")).as_deref(), Some("9"));
        assert_eq!(text(v.get("c")), None);
    }

    #[test]
    fn quotes_and_escapes_a_string() {
        assert_eq!(string("Red Rock"), "\"Red Rock\"");
        assert_eq!(string("Pier \"D\""), "\"Pier \\\"D\\\"\"");
        assert!(parse(&string("a\nb")).is_ok());
    }

    #[test]
    fn refuses_what_isnt_json() {
        assert!(parse("{").is_err());
        assert!(parse("").is_err());
    }
}

//! Domain-to-TLD resolution with IDN (punycode) normalization.
//!
//! TLDs are keyed by their ASCII A-label form (`xn--` for IDNs), so a lookup
//! must normalize the input the same way. `москва.рф` becomes `xn--80adxhks.xn--p1ai`,
//! and only then is the final label matched against the directory.

use idna::domain_to_ascii;

use crate::error::{Error, Result};

/// Trim surrounding whitespace and dots from a domain.
fn clean(domain: &str) -> &str {
    domain.trim().trim_matches('.')
}

/// Normalize `domain` to its lowercase ASCII A-label form.
///
/// ```
/// # use rdap_lookup::to_ascii;
/// assert_eq!(to_ascii("Nic.CH.").unwrap(), "nic.ch");
/// ```
pub fn to_ascii(domain: &str) -> Result<String> {
    let cleaned = clean(domain);
    if cleaned.is_empty() {
        return Err(Error::InvalidDomain(domain.to_string()));
    }
    domain_to_ascii(cleaned)
        .map(|ascii| ascii.to_ascii_lowercase())
        .map_err(|_| Error::InvalidDomain(domain.to_string()))
}

/// Resolve the A-label TLD (final label) of `domain`.
///
/// ```
/// # use rdap_lookup::resolve_tld;
/// assert_eq!(resolve_tld("www.example.com").unwrap(), "com");
/// ```
pub fn resolve_tld(domain: &str) -> Result<String> {
    let ascii = to_ascii(domain)?;
    match ascii.rsplit('.').next() {
        Some(tld) if !tld.is_empty() => Ok(tld.to_string()),
        _ => Err(Error::InvalidDomain(domain.to_string())),
    }
}

/// Whether `tld` is an IDN, i.e. an `xn--` A-label.
///
/// ```
/// # use rdap_lookup::is_idn;
/// assert!(is_idn("xn--p1ai"));
/// assert!(!is_idn("com"));
/// ```
pub fn is_idn(tld: &str) -> bool {
    clean(tld).to_ascii_lowercase().starts_with("xn--")
}

/// The Unicode (U-label) form of an IDN domain or label, if it is one.
///
/// ```
/// # use rdap_lookup::to_unicode;
/// assert_eq!(to_unicode("xn--p1ai").as_deref(), Some("рф"));
/// assert_eq!(to_unicode("com"), None);
/// ```
pub fn to_unicode(domain: &str) -> Option<String> {
    let cleaned = clean(domain);
    if !cleaned.to_ascii_lowercase().starts_with("xn--") {
        return None;
    }
    let (unicode, errors) = idna::domain_to_unicode(cleaned);
    if errors.is_err() || unicode == cleaned {
        None
    } else {
        Some(unicode)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_plain_domain() {
        assert_eq!(resolve_tld("example.com").unwrap(), "com");
        assert_eq!(resolve_tld("WWW.Example.CO.UK").unwrap(), "uk");
    }

    #[test]
    fn strips_trailing_dot() {
        assert_eq!(resolve_tld("example.com.").unwrap(), "com");
    }

    #[test]
    fn punycodes_idn() {
        assert_eq!(resolve_tld("пример.рф").unwrap(), "xn--p1ai");
        assert_eq!(to_ascii("пример.рф").unwrap(), "xn--e1afmkfd.xn--p1ai");
    }

    #[test]
    fn rejects_empty() {
        assert!(resolve_tld("..").is_err());
        assert!(resolve_tld("   ").is_err());
    }
}

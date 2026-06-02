//! `FingerprintRecord` — one library's identity claim.
//!
//! In-memory representation of `corpus/<library>.json` from
//! `kusari-sandbox/mikebom-fingerprints` (when loaded from cache) OR
//! the bundled in-source `FINGERPRINTS` seed (when external corpus is
//! disabled / unreachable). Same type backs both paths so the matcher
//! consumes a unified slice regardless of source.
//!
//! Schema versioned at v1 in
//! `kusari-sandbox/mikebom-fingerprints/schema/fingerprint-record.v1.json`.
//! mikebom-cli at load time treats records as TRUSTED (the sibling-repo
//! CI validated at PR time). The `validate()` method here is a thin
//! defensive check for the SHA-override path where an operator points
//! mikebom at an arbitrary commit that may not have passed CI.

use mikebom_common::types::purl::Purl;
use serde::Deserialize;
use thiserror::Error;

#[allow(dead_code)]
#[derive(Debug, Error)]
pub(crate) enum RecordValidationError {
    #[error("record has empty `library` field")]
    EmptyLibrary,
    #[error("record's `target_purl` failed parse: {raw}")]
    InvalidTargetPurl { raw: String },
    #[error("record's `symbols` list is empty")]
    EmptySymbols,
    #[error("record's `min_symbols` is zero")]
    ZeroMinSymbols,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct FingerprintRecord {
    pub library: String,
    pub target_purl: String,
    pub symbols: Vec<String>,
    pub min_symbols: u32,
    #[serde(default)]
    pub version_hint: Option<String>,
    #[serde(default)]
    pub variant: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[allow(dead_code)]
impl FingerprintRecord {
    /// FR-010 defensive validation. Sibling-repo CI catches these at
    /// PR time; this is the runtime fallback for SHA-override paths.
    pub fn validate(&self) -> Result<(), RecordValidationError> {
        if self.library.trim().is_empty() {
            return Err(RecordValidationError::EmptyLibrary);
        }
        if self.symbols.is_empty() {
            return Err(RecordValidationError::EmptySymbols);
        }
        if self.min_symbols == 0 {
            return Err(RecordValidationError::ZeroMinSymbols);
        }
        if Purl::new(&self.target_purl).is_err() {
            return Err(RecordValidationError::InvalidTargetPurl {
                raw: self.target_purl.clone(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
#[cfg_attr(test, allow(clippy::unwrap_used))]
mod tests {
    use super::*;

    fn minimal_valid_json() -> &'static str {
        r#"{
          "library": "openssl",
          "target_purl": "pkg:generic/openssl",
          "symbols": ["SSL_CTX_new", "SSL_new", "SSL_free"],
          "min_symbols": 2
        }"#
    }

    #[test]
    fn parses_minimal_valid_record() {
        let r: FingerprintRecord = serde_json::from_str(minimal_valid_json()).unwrap();
        assert_eq!(r.library, "openssl");
        assert_eq!(r.target_purl, "pkg:generic/openssl");
        assert_eq!(r.symbols.len(), 3);
        assert_eq!(r.min_symbols, 2);
        assert!(r.version_hint.is_none());
        assert!(r.variant.is_none());
        assert!(r.notes.is_none());
        r.validate().unwrap();
    }

    #[test]
    fn parses_record_with_optional_fields() {
        let json = r#"{
          "library": "openssl",
          "target_purl": "pkg:generic/openssl",
          "symbols": ["SSL_CTX_new", "SSL_new", "SSL_free"],
          "min_symbols": 2,
          "version_hint": ">=3.0",
          "variant": "libressl",
          "notes": "OpenBSD fork; ABI-compatible with OpenSSL"
        }"#;
        let r: FingerprintRecord = serde_json::from_str(json).unwrap();
        assert_eq!(r.version_hint.as_deref(), Some(">=3.0"));
        assert_eq!(r.variant.as_deref(), Some("libressl"));
        assert!(r.notes.is_some());
        r.validate().unwrap();
    }

    #[test]
    fn rejects_missing_required_field() {
        // Missing `min_symbols`.
        let json = r#"{
          "library": "openssl",
          "target_purl": "pkg:generic/openssl",
          "symbols": ["SSL_CTX_new"]
        }"#;
        assert!(serde_json::from_str::<FingerprintRecord>(json).is_err());
    }

    #[test]
    fn rejects_invalid_purl_in_target_purl() {
        let json = r#"{
          "library": "openssl",
          "target_purl": "not-a-purl",
          "symbols": ["SSL_CTX_new"],
          "min_symbols": 1
        }"#;
        let r: FingerprintRecord = serde_json::from_str(json).unwrap();
        assert!(matches!(
            r.validate(),
            Err(RecordValidationError::InvalidTargetPurl { .. })
        ));
    }

    #[test]
    fn rejects_zero_min_symbols() {
        let json = r#"{
          "library": "openssl",
          "target_purl": "pkg:generic/openssl",
          "symbols": ["SSL_CTX_new"],
          "min_symbols": 0
        }"#;
        let r: FingerprintRecord = serde_json::from_str(json).unwrap();
        assert!(matches!(
            r.validate(),
            Err(RecordValidationError::ZeroMinSymbols)
        ));
    }

    #[test]
    fn rejects_empty_symbols_list() {
        let json = r#"{
          "library": "openssl",
          "target_purl": "pkg:generic/openssl",
          "symbols": [],
          "min_symbols": 1
        }"#;
        let r: FingerprintRecord = serde_json::from_str(json).unwrap();
        assert!(matches!(
            r.validate(),
            Err(RecordValidationError::EmptySymbols)
        ));
    }
}

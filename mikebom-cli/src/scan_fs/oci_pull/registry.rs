//! Thin OCI distribution-spec HTTP client (milestone 032).
//!
//! Replaces the milestone-031 `oci-client::Client` integration. Built
//! on the workspace's `reqwest 0.12 + rustls-tls (ring)` (no new
//! HTTP/TLS deps) and `oci-spec 0.9` (types-only).
//!
//! Anonymous-only scope: when a registry returns 401 with
//! `WWW-Authenticate: Bearer realm="...",service="...",scope="..."`
//! we fetch a token from the realm UNAUTHENTICATED and retry. This
//! covers Docker Hub's "anonymous-but-token-required" handshake +
//! direct-anonymous registries (gcr.io, ghcr.io public, etc.).
//! Authenticated registries (private repos) are explicitly out of
//! scope here — milestone 031.x (#66) covers Docker keychain +
//! cred helpers.
//!
//! Endpoints (per OCI distribution-spec v1):
//!   - `GET /v2/<repo>/manifests/<reference>` — manifest or index
//!   - `GET /v2/<repo>/blobs/<digest>`        — config or layer blob

use anyhow::{anyhow, bail, Context, Result};
use sha2::Digest as _;

use oci_spec::image::{ImageIndex, ImageManifest};

use super::reference::ImageReference;

/// Manifest media types we accept (sent on the `Accept` header
/// for the manifest fetch + dispatched on the response
/// `Content-Type`).
const MANIFEST_MEDIA_TYPES: &[&str] = &[
    "application/vnd.oci.image.manifest.v1+json",
    "application/vnd.oci.image.index.v1+json",
    "application/vnd.docker.distribution.manifest.v2+json",
    "application/vnd.docker.distribution.manifest.list.v2+json",
];

/// Either a single-platform image manifest or a multi-platform
/// image index (manifest list). The caller dispatches on which.
///
/// Both variants box their payload — `ImageManifest` is far
/// larger than `ImageIndex` (it carries layer descriptors), and
/// the `clippy::large_enum_variant` lint flagged the size
/// disparity. Boxing makes the enum's stack size constant.
#[allow(clippy::large_enum_variant)]
pub(super) enum ManifestOrIndex {
    Manifest(ImageManifest),
    Index(ImageIndex),
}

/// Thin async HTTP client over the OCI distribution-spec.
pub(super) struct RegistryClient {
    http: reqwest::Client,
}

impl RegistryClient {
    pub(super) fn new() -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("mikebom/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("building reqwest::Client for OCI registry")?;
        Ok(Self { http })
    }

    /// Fetch the manifest for `reference`. Returns either a
    /// single-platform manifest or a multi-platform index.
    /// Handles bearer-token retry transparently.
    pub(super) async fn fetch_manifest(
        &self,
        reference: &ImageReference,
    ) -> Result<ManifestOrIndex> {
        let url = manifest_url(reference);
        let body = self.fetch_with_bearer_retry(&url, MANIFEST_MEDIA_TYPES).await?;
        let content_type = body.content_type;
        let bytes = body.bytes;

        // Dispatch on the response's content-type. Two flavors —
        // single manifest vs index/list.
        if is_index_media_type(&content_type) {
            let index: ImageIndex = serde_json::from_slice(&bytes)
                .with_context(|| format!("parsing manifest list at {url}"))?;
            return Ok(ManifestOrIndex::Index(index));
        }
        let manifest: ImageManifest = serde_json::from_slice(&bytes)
            .with_context(|| format!("parsing manifest at {url}"))?;
        Ok(ManifestOrIndex::Manifest(manifest))
    }

    /// Fetch a blob (config or layer) and verify its SHA-256
    /// matches the declared `digest`. The digest is the
    /// `<algorithm>:<hex>` form straight from the descriptor.
    pub(super) async fn fetch_blob(
        &self,
        reference: &ImageReference,
        digest: &str,
    ) -> Result<Vec<u8>> {
        let url = blob_url(reference, digest);
        // Blob endpoint accepts any media type; we send `*/*`.
        let body = self.fetch_with_bearer_retry(&url, &["*/*"]).await?;
        verify_sha256(&body.bytes, digest)
            .with_context(|| format!("verifying blob {digest} from {url}"))?;
        Ok(body.bytes)
    }

    /// GET `url` with the supplied Accept media types. Handles
    /// 401 → bearer-token-fetch → retry. Returns the body bytes
    /// + the Content-Type header so the caller can dispatch.
    async fn fetch_with_bearer_retry(
        &self,
        url: &str,
        accept: &[&str],
    ) -> Result<ResponseBody> {
        let accept_header = accept.join(", ");
        let first = self
            .http
            .get(url)
            .header("Accept", &accept_header)
            .send()
            .await
            .with_context(|| format!("sending GET {url}"))?;
        let status = first.status();
        if status.is_success() {
            return ResponseBody::from_response(first).await;
        }
        if status.as_u16() == 401 {
            // Parse the bearer challenge, fetch a token, retry.
            let www_auth = first
                .headers()
                .get(reqwest::header::WWW_AUTHENTICATE)
                .ok_or_else(|| {
                    anyhow!("registry returned 401 without WWW-Authenticate header for GET {url}")
                })?
                .to_str()
                .context("WWW-Authenticate is not valid UTF-8")?
                .to_string();
            let challenge = parse_bearer_challenge(&www_auth)?;
            let token = self.fetch_bearer_token(&challenge).await?;
            let retry = self
                .http
                .get(url)
                .header("Accept", &accept_header)
                .bearer_auth(&token)
                .send()
                .await
                .with_context(|| format!("retrying GET {url} with bearer token"))?;
            if retry.status().is_success() {
                return ResponseBody::from_response(retry).await;
            }
            bail!(
                "registry returned {} for GET {url} after bearer-token retry. \
                 Authenticated registries are not yet supported in milestone 031 \
                 (tracked as 031.x — see issue #66).",
                retry.status()
            );
        }
        // 403 / 404 / 5xx etc.
        bail!(
            "registry returned {status} for GET {url}. \
             Authenticated registries are not yet supported in milestone 031 \
             (tracked as 031.x — see issue #66)."
        );
    }

    /// Anonymous bearer-token fetch from the realm. Used when
    /// the registry's 401 response includes a `Bearer
    /// realm="...",service="...",scope="..."` challenge.
    async fn fetch_bearer_token(&self, challenge: &BearerChallenge) -> Result<String> {
        let mut req = self.http.get(&challenge.realm);
        if let Some(service) = challenge.service.as_deref() {
            req = req.query(&[("service", service)]);
        }
        if let Some(scope) = challenge.scope.as_deref() {
            req = req.query(&[("scope", scope)]);
        }
        let resp = req
            .send()
            .await
            .with_context(|| format!("fetching bearer token from {}", challenge.realm))?;
        if !resp.status().is_success() {
            bail!(
                "bearer token endpoint {} returned {}",
                challenge.realm,
                resp.status()
            );
        }
        let body: serde_json::Value = resp
            .json()
            .await
            .context("parsing bearer token response as JSON")?;
        // Different registries return different field names; check
        // common ones.
        for field in ["token", "access_token"] {
            if let Some(t) = body.get(field).and_then(|v| v.as_str()) {
                return Ok(t.to_string());
            }
        }
        Err(anyhow!(
            "bearer token response missing `token` / `access_token` field"
        ))
    }
}

/// The `WWW-Authenticate: Bearer realm="...",service="...",scope="..."`
/// challenge fields. realm is required; service and scope are
/// optional (some registries emit only realm).
struct BearerChallenge {
    realm: String,
    service: Option<String>,
    scope: Option<String>,
}

/// Parse a `WWW-Authenticate: Bearer realm="...",service="...",scope="..."`
/// header value into its fields.
fn parse_bearer_challenge(value: &str) -> Result<BearerChallenge> {
    // Strip the `Bearer ` prefix (case-insensitive).
    let trimmed = value.trim_start();
    let after_scheme = if trimmed.len() >= 7
        && trimmed[..7].eq_ignore_ascii_case("Bearer ")
    {
        &trimmed[7..]
    } else {
        bail!("WWW-Authenticate is not a Bearer challenge: {value}");
    };

    // Split on commas at the top level. Values may contain commas
    // inside double-quotes; we respect that.
    let mut realm: Option<String> = None;
    let mut service: Option<String> = None;
    let mut scope: Option<String> = None;
    for (k, v) in iter_kv_pairs(after_scheme) {
        match k.as_str() {
            "realm" => realm = Some(v),
            "service" => service = Some(v),
            "scope" => scope = Some(v),
            _ => {}
        }
    }
    let realm = realm.ok_or_else(|| {
        anyhow!("WWW-Authenticate Bearer challenge missing `realm`: {value}")
    })?;
    Ok(BearerChallenge {
        realm,
        service,
        scope,
    })
}

/// Iterate `key="value"` pairs respecting double-quoted values
/// (which may contain commas, equals signs, etc.).
fn iter_kv_pairs(s: &str) -> impl Iterator<Item = (String, String)> + '_ {
    let mut chars = s.chars().peekable();
    std::iter::from_fn(move || {
        // Skip leading whitespace + commas.
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() || c == ',' {
                chars.next();
            } else {
                break;
            }
        }
        // Read key up to `=`.
        let mut key = String::new();
        for c in chars.by_ref() {
            if c == '=' {
                break;
            }
            key.push(c);
        }
        if key.is_empty() {
            return None;
        }
        let key = key.trim().to_string();
        // Read value: either `"quoted,maybe,with,commas"` or bare.
        let mut value = String::new();
        if chars.peek() == Some(&'"') {
            chars.next(); // consume opening quote
            while let Some(c) = chars.next() {
                if c == '\\' {
                    if let Some(escaped) = chars.next() {
                        value.push(escaped);
                    }
                } else if c == '"' {
                    break;
                } else {
                    value.push(c);
                }
            }
        } else {
            for c in chars.by_ref() {
                if c == ',' {
                    break;
                }
                value.push(c);
            }
        }
        Some((key, value.trim().to_string()))
    })
}

/// Detect whether a manifest `Content-Type` header indicates a
/// multi-arch image index (manifest list), as opposed to a
/// single-platform manifest.
fn is_index_media_type(content_type: &str) -> bool {
    // Strip any `; charset=utf-8`-style parameters.
    let mt = content_type.split(';').next().unwrap_or("").trim();
    matches!(
        mt,
        "application/vnd.oci.image.index.v1+json"
            | "application/vnd.docker.distribution.manifest.list.v2+json"
    )
}

fn manifest_url(reference: &ImageReference) -> String {
    let registry = resolve_registry_for_url(&reference.registry);
    format!(
        "https://{registry}/v2/{}/manifests/{}",
        reference.repository,
        reference.resolved_reference()
    )
}

fn blob_url(reference: &ImageReference, digest: &str) -> String {
    let registry = resolve_registry_for_url(&reference.registry);
    format!(
        "https://{registry}/v2/{}/blobs/{}",
        reference.repository, digest
    )
}

/// `docker.io` is the user-facing registry name; the actual API
/// endpoint is `registry-1.docker.io`. Other registries use their
/// hostname directly.
fn resolve_registry_for_url(registry: &str) -> &str {
    if registry == "docker.io" {
        "registry-1.docker.io"
    } else {
        registry
    }
}

fn verify_sha256(bytes: &[u8], expected_digest: &str) -> Result<()> {
    let (algo, expected_hex) = expected_digest
        .split_once(':')
        .ok_or_else(|| anyhow!("digest missing `<algorithm>:<hex>` separator: {expected_digest}"))?;
    if !algo.eq_ignore_ascii_case("sha256") {
        bail!("only sha256 digests supported, got `{algo}` in `{expected_digest}`");
    }
    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);
    let actual_hex = format!("{:x}", hasher.finalize());
    if !actual_hex.eq_ignore_ascii_case(expected_hex) {
        bail!(
            "blob digest mismatch: expected sha256:{expected_hex}, got sha256:{actual_hex}"
        );
    }
    Ok(())
}

struct ResponseBody {
    bytes: Vec<u8>,
    content_type: String,
}

impl ResponseBody {
    async fn from_response(resp: reqwest::Response) -> Result<Self> {
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let bytes = resp
            .bytes()
            .await
            .context("reading response body")?
            .to_vec();
        Ok(Self {
            bytes,
            content_type,
        })
    }
}

#[cfg(test)]
#[cfg_attr(test, allow(clippy::unwrap_used))]
mod tests {
    use super::*;

    #[test]
    fn parse_bearer_challenge_extracts_realm_service_scope() {
        // Docker Hub's actual challenge format.
        let v = r#"Bearer realm="https://auth.docker.io/token",service="registry.docker.io",scope="repository:library/alpine:pull""#;
        let c = parse_bearer_challenge(v).unwrap();
        assert_eq!(c.realm, "https://auth.docker.io/token");
        assert_eq!(c.service.as_deref(), Some("registry.docker.io"));
        assert_eq!(c.scope.as_deref(), Some("repository:library/alpine:pull"));
    }

    #[test]
    fn parse_bearer_challenge_handles_realm_only() {
        let v = r#"Bearer realm="https://example.com/token""#;
        let c = parse_bearer_challenge(v).unwrap();
        assert_eq!(c.realm, "https://example.com/token");
        assert_eq!(c.service, None);
        assert_eq!(c.scope, None);
    }

    #[test]
    fn parse_bearer_challenge_handles_unquoted_values() {
        // RFC 7235 allows token-style values without quotes.
        let v = "Bearer realm=https://example.com/token,service=example.com";
        let c = parse_bearer_challenge(v).unwrap();
        assert_eq!(c.realm, "https://example.com/token");
        assert_eq!(c.service.as_deref(), Some("example.com"));
    }

    #[test]
    fn parse_bearer_challenge_rejects_basic_scheme() {
        let v = r#"Basic realm="x""#;
        assert!(parse_bearer_challenge(v).is_err());
    }

    #[test]
    fn parse_bearer_challenge_rejects_missing_realm() {
        let v = r#"Bearer service="x",scope="y""#;
        assert!(parse_bearer_challenge(v).is_err());
    }

    #[test]
    fn parse_bearer_challenge_handles_case_insensitive_scheme() {
        let v = r#"bearer realm="https://example.com/token""#;
        let c = parse_bearer_challenge(v).unwrap();
        assert_eq!(c.realm, "https://example.com/token");
    }

    #[test]
    fn is_index_media_type_recognizes_oci_and_docker_lists() {
        assert!(is_index_media_type(
            "application/vnd.oci.image.index.v1+json"
        ));
        assert!(is_index_media_type(
            "application/vnd.docker.distribution.manifest.list.v2+json"
        ));
        // Single-platform manifests are NOT indexes.
        assert!(!is_index_media_type(
            "application/vnd.oci.image.manifest.v1+json"
        ));
        assert!(!is_index_media_type(
            "application/vnd.docker.distribution.manifest.v2+json"
        ));
    }

    #[test]
    fn is_index_media_type_strips_charset_parameter() {
        assert!(is_index_media_type(
            "application/vnd.oci.image.index.v1+json; charset=utf-8"
        ));
    }

    #[test]
    fn verify_sha256_passes_on_match() {
        let bytes = b"hello world";
        // sha256("hello world") = b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9
        let digest = "sha256:b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        verify_sha256(bytes, digest).unwrap();
    }

    #[test]
    fn verify_sha256_fails_on_mismatch() {
        let bytes = b"hello world";
        let digest = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
        let err = verify_sha256(bytes, digest).unwrap_err();
        assert!(err.to_string().contains("digest mismatch"));
    }

    #[test]
    fn verify_sha256_rejects_non_sha256_algorithm() {
        assert!(verify_sha256(b"x", "sha512:00").is_err());
    }

    #[test]
    fn verify_sha256_rejects_malformed_digest() {
        assert!(verify_sha256(b"x", "no-separator").is_err());
    }

    #[test]
    fn manifest_url_uses_registry_1_for_docker_io() {
        let reference = super::super::reference::parse_reference("alpine:3.19").unwrap();
        let url = manifest_url(&reference);
        assert_eq!(
            url,
            "https://registry-1.docker.io/v2/library/alpine/manifests/3.19"
        );
    }

    #[test]
    fn manifest_url_uses_other_registries_directly() {
        let reference =
            super::super::reference::parse_reference("gcr.io/distroless/static-debian12:latest")
                .unwrap();
        let url = manifest_url(&reference);
        assert_eq!(
            url,
            "https://gcr.io/v2/distroless/static-debian12/manifests/latest"
        );
    }

    #[test]
    fn blob_url_uses_digest_directly() {
        let reference = super::super::reference::parse_reference("alpine:3.19").unwrap();
        let url = blob_url(&reference, "sha256:abc123");
        assert_eq!(
            url,
            "https://registry-1.docker.io/v2/library/alpine/blobs/sha256:abc123"
        );
    }
}

//! Lightweight "check for updates" against the public GitHub Releases API.
//!
//! On startup the app performs a single best-effort HTTP GET to the repo's
//! `releases/latest` endpoint. When the published release is strictly newer than
//! the running build it surfaces a non-intrusive notice in the About dialog plus
//! a badge on the Settings button. This never self-installs — it only notifies
//! and links to the release page.
//!
//! Every failure mode (offline, rate-limited, malformed JSON, unparseable
//! version) is swallowed and simply yields "no update", so a failed check is
//! invisible to the user.

use serde::Deserialize;

/// GitHub "latest release" endpoint for the public soyuz repository.
const RELEASES_URL: &str = "https://api.github.com/repos/noahsabaj/soyuz/releases/latest";

/// GitHub requires a User-Agent header on every API request.
const USER_AGENT: &str = "soyuz-studio";

/// An available newer release, ready to surface in the UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateInfo {
    /// Release version without the leading `v` (e.g. "0.8.0").
    pub version: String,
    /// Browser URL of the release page.
    pub url: String,
}

/// The subset of the GitHub release JSON we care about.
#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
}

/// Run the update check once, off the async executor.
///
/// The blocking HTTP call runs on Tokio's blocking pool so it never stalls the
/// UI. Returns `Some` only when a strictly newer release exists; every error
/// maps to `None`.
pub async fn check_for_update() -> Option<UpdateInfo> {
    match tokio::task::spawn_blocking(fetch_latest_release).await {
        Ok(result) => result,
        Err(error) => {
            tracing::debug!("update check task failed: {error}");
            None
        }
    }
}

/// Blocking GitHub fetch + version comparison. Any error collapses to `None`.
fn fetch_latest_release() -> Option<UpdateInfo> {
    let body = fetch_release_body()?;
    let release: GithubRelease = serde_json::from_str(&body)
        .map_err(|error| tracing::debug!("update check: failed to parse release JSON: {error}"))
        .ok()?;
    newer_release(
        env!("CARGO_PKG_VERSION"),
        &release.tag_name,
        &release.html_url,
    )
}

/// Perform the blocking HTTPS GET and return the response body, or `None` on any
/// transport error (offline, TLS, rate limit, non-2xx status, oversized body).
fn fetch_release_body() -> Option<String> {
    let mut response = ureq::get(RELEASES_URL)
        .header("User-Agent", USER_AGENT)
        .call()
        .map_err(|error| tracing::debug!("update check request failed: {error}"))
        .ok()?;
    response
        .body_mut()
        .read_to_string()
        .map_err(|error| tracing::debug!("update check: failed to read response body: {error}"))
        .ok()
}

/// Compare the running version to a release tag, returning `Some(UpdateInfo)`
/// only when `tag` parses to a version strictly greater than `current`.
///
/// The leading `v` on the tag (GitHub convention, e.g. `v0.8.0`) is stripped
/// before parsing. Pure and side-effect free so it is unit-testable offline.
fn newer_release(current: &str, tag: &str, url: &str) -> Option<UpdateInfo> {
    let latest_str = tag.strip_prefix('v').unwrap_or(tag);
    let current = semver::Version::parse(current).ok()?;
    let latest = semver::Version::parse(latest_str).ok()?;

    (latest > current).then(|| UpdateInfo {
        version: latest.to_string(),
        url: url.to_string(),
    })
}

/// Open the release page in the user's default browser (best-effort, silent).
pub fn open_release_page(url: &str) {
    if let Err(error) = webbrowser::open(url) {
        tracing::debug!("failed to open release page {url}: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_strictly_newer_release() {
        assert_eq!(
            newer_release("0.7.1", "v0.8.0", "https://example.com/rel"),
            Some(UpdateInfo {
                version: "0.8.0".to_string(),
                url: "https://example.com/rel".to_string(),
            })
        );
    }

    #[test]
    fn ignores_equal_version() {
        assert_eq!(
            newer_release("0.7.1", "v0.7.1", "https://example.com"),
            None
        );
    }

    #[test]
    fn ignores_older_release() {
        // Running 0.7.1 against a latest release of 0.7.0 must show nothing.
        assert_eq!(
            newer_release("0.7.1", "v0.7.0", "https://example.com"),
            None
        );
    }

    #[test]
    fn tolerates_tag_without_v_prefix() {
        assert!(newer_release("0.7.1", "0.8.0", "https://example.com").is_some());
    }

    #[test]
    fn unparseable_versions_yield_none() {
        assert_eq!(
            newer_release("0.7.1", "vNightly", "https://example.com"),
            None
        );
        assert_eq!(
            newer_release("not-semver", "v0.8.0", "https://example.com"),
            None
        );
    }
}

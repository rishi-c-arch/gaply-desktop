//! **Every external link the webview opens, rechecked in Rust** (Phase 2
//! evaluation fixes, part C follow-up, item 7).
//!
//! The opener plugin used to open links for the webview on its own: its
//! injected click handler sent any `<a target="_blank">` to `open_url`, and
//! the window's `opener:default` allowed every `http(s)`, `mailto` and `tel`
//! URL, so a page could open any link with no check here. Now the plugin's
//! handler is off, the window may only reveal a file in its folder, and the
//! frontend's own handler (`src/services/externalLinks.ts`) and the OAuth
//! sign-in send links to [`open_external_link`]. Level 1's model-supplied
//! sources keep their stricter command, `stats_open_source_link`, with its
//! domain list.
//!
//! The check: `https` only, a host, no user name or password, no whitespace
//! or control character, at most [`MAX_LINK_CHARS`] characters. A refusal
//! names its reason as a closed word and never the URL.

use gaply_core::GaplyError;

/// The longest link opened. An OAuth authorize URL is a few hundred
/// characters; a journal page, fewer.
pub const MAX_LINK_CHARS: usize = 2048;

/// Why a link was not opened. Closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkRefusal {
    TooLong,
    Malformed,
    NotHttps,
    Credentials,
}

impl LinkRefusal {
    pub fn word(self) -> &'static str {
        match self {
            LinkRefusal::TooLong => "too_long",
            LinkRefusal::Malformed => "malformed",
            LinkRefusal::NotHttps => "not_https",
            LinkRefusal::Credentials => "credentials",
        }
    }
}

/// The link, rechecked, as it will be opened.
pub fn checked_external_link(raw: &str) -> Result<url::Url, LinkRefusal> {
    if raw.chars().count() > MAX_LINK_CHARS {
        return Err(LinkRefusal::TooLong);
    }
    if raw.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(LinkRefusal::Malformed);
    }
    let url = url::Url::parse(raw).map_err(|_| LinkRefusal::Malformed)?;
    if url.scheme() != "https" {
        return Err(LinkRefusal::NotHttps);
    }
    if url.host_str().map_or(true, str::is_empty) {
        return Err(LinkRefusal::Malformed);
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(LinkRefusal::Credentials);
    }
    Ok(url)
}

/// Open `url` in the system browser, after [`checked_external_link`].
#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn open_external_link(app: tauri::AppHandle, url: String) -> Result<(), GaplyError> {
    use tauri_plugin_opener::OpenerExt;
    let url = checked_external_link(&url).map_err(|r| {
        tracing::info!(reason = r.word(), "external link refused");
        GaplyError::Validation(format!(
            "{{\"code\":\"link_refused\",\"reason\":\"{}\"}}",
            r.word()
        ))
    })?;
    app.opener()
        .open_url(url.as_str(), None::<&str>)
        .map_err(|_| GaplyError::Validation("{\"code\":\"link_not_opened\"}".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_plain_https_link_is_opened() {
        for ok in ["https://example.org/", "https://x.supabase.co/auth/v1/authorize?provider=google&redirect_to=gaply%3A%2F%2Fauth", "https://doi.org/10.1/abc#frag"] {
            assert!(checked_external_link(ok).is_ok(), "{ok}");
        }
        let cases = [
            ("http://example.org/", LinkRefusal::NotHttps),
            ("mailto:a@example.org", LinkRefusal::NotHttps),
            ("tel:+100", LinkRefusal::NotHttps),
            ("javascript:alert(1)", LinkRefusal::NotHttps),
            ("file:///etc/passwd", LinkRefusal::NotHttps),
            ("data:text/html,x", LinkRefusal::NotHttps),
            ("gaply://auth", LinkRefusal::NotHttps),
            ("https://user:pw@example.org/", LinkRefusal::Credentials),
            ("https://user@example.org/", LinkRefusal::Credentials),
            ("https://exa mple.org/", LinkRefusal::Malformed),
            ("https://example.org/\u{0007}", LinkRefusal::Malformed),
            ("https://", LinkRefusal::Malformed),
            ("not a url", LinkRefusal::Malformed),
        ];
        for (url, why) in cases {
            assert_eq!(checked_external_link(url).map(|_| ()), Err(why), "{url}");
        }
        let long = format!("https://example.org/{}", "a".repeat(MAX_LINK_CHARS));
        assert_eq!(
            checked_external_link(&long).map(|_| ()),
            Err(LinkRefusal::TooLong)
        );
    }
}

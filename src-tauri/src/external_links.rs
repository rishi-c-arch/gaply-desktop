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
//! The check: `https` with a host and no user name or password, or (part C
//! follow-up 2) `mailto` to an address in [`MAILTO_ADDRESSES`] with no
//! parameter but one `subject`; no whitespace or control character, raw or
//! percent-decoded in a subject; at most [`MAX_LINK_CHARS`] characters. http,
//! tel and every other scheme are refused. A refusal names its reason as a
//! closed word and never the URL.

use gaply_core::GaplyError;

/// The longest link opened. An OAuth authorize URL is a few hundred
/// characters; a journal page, fewer.
pub const MAX_LINK_CHARS: usize = 2048;

/// The only addresses a `mailto` link may open to: the ones the app's own
/// pages link to (`SupportPage.tsx` and `ContactPage.tsx`; `CareerPage.tsx`
/// adds a body, which is refused). Compared without case.
pub const MAILTO_ADDRESSES: &[&str] = &["helloresearcher@gaply.in"];

/// Why a link was not opened. Closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkRefusal {
    TooLong,
    Malformed,
    /// Neither https nor mailto (http, tel, and every other scheme).
    NotHttps,
    Credentials,
    /// A mailto whose recipients are not exactly one listed address.
    MailtoAddressNotListed,
    /// A mailto with a parameter other than one `subject`.
    MailtoParameterNotAllowed,
}

impl LinkRefusal {
    pub fn word(self) -> &'static str {
        match self {
            LinkRefusal::TooLong => "too_long",
            LinkRefusal::Malformed => "malformed",
            LinkRefusal::NotHttps => "not_https",
            LinkRefusal::Credentials => "credentials",
            LinkRefusal::MailtoAddressNotListed => "mailto_address_not_listed",
            LinkRefusal::MailtoParameterNotAllowed => "mailto_parameter_not_allowed",
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
    if url.scheme() == "mailto" {
        return checked_mailto(url);
    }
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

/// A mailto: exactly one listed address as the whole path, at most one
/// `subject` parameter with no control character once decoded, no fragment.
fn checked_mailto(url: url::Url) -> Result<url::Url, LinkRefusal> {
    if url.fragment().is_some() {
        return Err(LinkRefusal::Malformed);
    }
    let to = url.path().to_ascii_lowercase();
    if !MAILTO_ADDRESSES.iter().any(|a| *a == to) {
        return Err(LinkRefusal::MailtoAddressNotListed);
    }
    let mut subjects = 0;
    for (key, value) in url.query_pairs() {
        if key != "subject" {
            return Err(LinkRefusal::MailtoParameterNotAllowed);
        }
        subjects += 1;
        if value.chars().any(char::is_control) {
            return Err(LinkRefusal::Malformed);
        }
    }
    if subjects > 1 {
        return Err(LinkRefusal::MailtoParameterNotAllowed);
    }
    Ok(url)
}

/// Check `raw` and hand it to `open`, which opens it in the system browser
/// or mail client: the whole command except the plugin call, so a test can
/// show what reaches the opener without launching anything.
pub fn open_checked(
    raw: &str,
    open: impl FnOnce(&str) -> Result<(), ()>,
) -> Result<(), GaplyError> {
    let url = checked_external_link(raw).map_err(|r| {
        tracing::info!(reason = r.word(), "external link refused");
        GaplyError::Validation(format!(
            "{{\"code\":\"link_refused\",\"reason\":\"{}\"}}",
            r.word()
        ))
    })?;
    open(url.as_str())
        .map_err(|_| GaplyError::Validation("{\"code\":\"link_not_opened\"}".to_string()))
}

/// Open `url` in the system browser, after [`checked_external_link`].
#[tauri::command]
#[tracing::instrument(skip_all)]
pub async fn open_external_link(app: tauri::AppHandle, url: String) -> Result<(), GaplyError> {
    use tauri_plugin_opener::OpenerExt;
    open_checked(&url, |u| {
        app.opener().open_url(u, None::<&str>).map_err(|_| ())
    })
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
            ("mailto:a@example.org", LinkRefusal::MailtoAddressNotListed),
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
    /// Part C follow-up 2: mailto opens, but only to an address the app's own
    /// pages link to, with no parameter but a subject. Everything else is a
    /// closed refusal; http and tel stay refused.
    #[test]
    fn mailto_opens_only_to_a_listed_address_with_at_most_a_subject() {
        for ok in [
            "mailto:helloresearcher@gaply.in",
            "mailto:helloresearcher@gaply.in?subject=Support%20request",
            "mailto:HelloResearcher@Gaply.in",
        ] {
            assert!(checked_external_link(ok).is_ok(), "{ok}");
        }
        let refused = [
            ("mailto:someone@example.org", "mailto_address_not_listed"),
            (
                "mailto:helloresearcher@gaply.in,someone@example.org",
                "mailto_address_not_listed",
            ),
            (
                "mailto:?to=helloresearcher@gaply.in",
                "mailto_address_not_listed",
            ),
            (
                "mailto:helloresearcher@gaply.in?body=hi",
                "mailto_parameter_not_allowed",
            ),
            (
                "mailto:helloresearcher@gaply.in?subject=a&cc=x@example.org",
                "mailto_parameter_not_allowed",
            ),
            (
                "mailto:helloresearcher@gaply.in?subject=a&subject=b",
                "mailto_parameter_not_allowed",
            ),
            (
                "mailto:helloresearcher@gaply.in?subject=a%0Abcc:x@example.org",
                "malformed",
            ),
            ("mailto:helloresearcher@gaply.in#x", "malformed"),
            ("tel:+916387144799", "not_https"),
            ("http://gaply.in/", "not_https"),
        ];
        for (url, word) in refused {
            assert_eq!(
                checked_external_link(url)
                    .map(|_| ())
                    .map_err(LinkRefusal::word),
                Err(word),
                "{url}"
            );
        }
    }

    /// The Google sign-in URL as supabase-js 2.110 builds it (auth-js
    /// `_getUrlForProvider`: provider, redirect_to, the PKCE challenge of 43
    /// characters and its method, the app's query parameters, and
    /// skip_http_redirect), for a project URL of the real shape (a 20-character
    /// ref). The vitest `signInLink.vitest.ts` measures the URL supabase-js
    /// itself builds; this pins that the check admits one twice as long.
    #[test]
    fn the_sign_in_url_is_admitted_with_at_least_twice_its_length_to_spare() {
        let url = format!(
            "https://{}.supabase.co/auth/v1/authorize?provider=google&redirect_to={}&code_challenge={}&code_challenge_method=s256&access_type=offline&prompt=consent&skip_http_redirect=true",
            "a".repeat(20),
            "gaply%3A%2F%2Fauth%2Fcallback",
            "A".repeat(43)
        );
        assert!(checked_external_link(&url).is_ok());
        let doubled = format!("{url}&pad={}", "x".repeat(url.chars().count()));
        assert!(doubled.chars().count() >= 2 * url.chars().count());
        assert!(
            checked_external_link(&doubled).is_ok(),
            "{} characters",
            doubled.chars().count()
        );
    }

    /// What `open_external_link` hands to the opener: the sign-in URL and a
    /// listed mailto reach it; a refused link never does.
    #[test]
    fn the_command_hands_checked_links_to_the_opener_and_nothing_else() {
        let sign_in = format!("https://{}.supabase.co/auth/v1/authorize?provider=google&redirect_to=gaply%3A%2F%2Fauth%2Fcallback&code_challenge={}&code_challenge_method=s256&access_type=offline&prompt=consent&skip_http_redirect=true", "a".repeat(20), "A".repeat(43));
        for (url, reaches) in [
            (sign_in.as_str(), true),
            ("mailto:helloresearcher@gaply.in?subject=Support", true),
            ("mailto:someone@example.org", false),
            ("http://gaply.in/", false),
            ("tel:+100", false),
        ] {
            let mut opened = Vec::new();
            let r = open_checked(url, |u| {
                opened.push(u.to_string());
                Ok(())
            });
            assert_eq!(r.is_ok(), reaches, "{url}: {r:?}");
            assert_eq!(opened.len(), usize::from(reaches), "{url}");
            if reaches {
                assert_eq!(opened[0], url);
            }
        }
    }
}

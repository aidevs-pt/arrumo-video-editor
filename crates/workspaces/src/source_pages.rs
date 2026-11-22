//! Reads public web pages the creator points Arrumo at, such as a website or channel, and keeps
//! only their readable text. Only public addresses are contacted, each hop is checked, and the
//! connection is pinned to the checked address so DNS cannot redirect it to a local service.

use std::{
    io::Read,
    net::{IpAddr, SocketAddr, ToSocketAddrs},
    time::Duration,
};

use url::Url;

pub(crate) const MAX_SOURCES: usize = 8;
const MAX_PAGE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_PAGE_CHARS: usize = 20_000;
const MAX_REDIRECTS: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourcePage {
    pub url: String,
    pub title: String,
    pub description: String,
    pub text: String,
    /// Images the page shows or advertises, with absolute addresses.
    pub images: Vec<PageImage>,
    /// The page's declared theme colour, as `#rrggbb`.
    pub theme_color: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub(crate) struct PageImage {
    pub url: String,
    pub alt: String,
    /// Where it came from: `social` (share image), `icon`, `video_thumbnail` or `image`.
    pub role: &'static str,
}

const MAX_IMAGES: usize = 40;

/// Collects the page's images: share images, icons, video thumbnails and `<img>` elements,
/// resolved against `base` and limited to public HTTPS addresses.
pub(crate) fn page_images(html: &str, base: &Url) -> Vec<PageImage> {
    let lower = html.to_ascii_lowercase();
    let mut found: Vec<PageImage> = Vec::new();
    let add = |raw: &str, alt: String, role: &'static str, found: &mut Vec<PageImage>| {
        let raw = decode(raw.trim());
        let Ok(url) = base.join(&raw) else { return };
        if url.scheme() != "https"
            || url.as_str().len() > 1024
            || found.iter().any(|image| image.url == url.as_str())
        {
            return;
        }
        if found.len() < MAX_IMAGES {
            found.push(PageImage {
                url: url.to_string(),
                alt: collapse(&alt).chars().take(200).collect(),
                role,
            });
        }
    };
    for name in ["og:image", "twitter:image", "og:image:url"] {
        if let Some(url) = meta_content(html, &lower, name) {
            add(&url, String::new(), "social", &mut found);
        }
    }
    let mut from = 0;
    while let Some(offset) = lower[from..].find("<link") {
        let start = from + offset;
        let end = lower[start..]
            .find('>')
            .map_or(lower.len(), |close| start + close);
        let tag = &lower[start..end];
        if tag.contains("icon")
            && let Some(href) = attribute(&html[start..end], &lower[start..end], "href")
        {
            add(&href, String::new(), "icon", &mut found);
        }
        from = end;
    }
    // Video platforms list their thumbnails in scripts rather than <img> elements.
    let mut from = 0;
    while let Some(offset) = html[from..].find("i.ytimg.com/vi/") {
        let start = from + offset;
        let id: String = html[start + 15..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .collect();
        if id.len() == 11 {
            add(
                &format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg"),
                String::new(),
                "video_thumbnail",
                &mut found,
            );
        }
        from = start + 15;
    }
    let mut from = 0;
    while let Some(offset) = lower[from..].find("<img") {
        let start = from + offset;
        let end = lower[start..]
            .find('>')
            .map_or(lower.len(), |close| start + close);
        let (tag, original) = (&lower[start..end], &html[start..end]);
        if let Some(src) = attribute(original, tag, "src").filter(|src| !src.starts_with("data:")) {
            let alt = attribute(original, tag, "alt").unwrap_or_default();
            add(&src, alt, "image", &mut found);
        }
        from = end;
    }
    found
}

/// A quoted attribute's value from one tag.
fn attribute(original: &str, lower: &str, name: &str) -> Option<String> {
    let mut from = 0;
    while let Some(offset) = lower[from..].find(&format!("{name}=")) {
        let at = from + offset;
        let boundary = at == 0 || lower[..at].ends_with([' ', '\n', '\t', '\r']);
        let value_start = at + name.len() + 1;
        if boundary {
            let quote = original[value_start..].chars().next()?;
            if quote == '"' || quote == '\'' {
                let value = &original[value_start + 1..];
                return value.find(quote).map(|close| value[..close].to_owned());
            }
        }
        from = value_start;
    }
    None
}

/// Why a source could not be read, as a code the interface translates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceProblem {
    InvalidAddress,
    NotPublic,
    NotFound,
    Unreachable,
    TimedOut,
    HttpError,
    NotAPage,
    NoText,
    TooManyRedirects,
}

impl std::fmt::Display for SourceProblem {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for SourceProblem {}

/// Accepts what people type ("example.com", "https://…") and returns a web URL.
pub(crate) fn source_url(input: &str) -> Result<Url, SourceProblem> {
    let input = input.trim();
    let candidate = if input.contains("://") {
        input.to_owned()
    } else {
        format!("https://{input}")
    };
    let url = Url::parse(&candidate).map_err(|_| SourceProblem::InvalidAddress)?;
    if !matches!(url.scheme(), "https" | "http")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(SourceProblem::InvalidAddress);
    }
    Ok(url)
}

pub(crate) fn public_address(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            !(v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_multicast()
                || a == 0
                || a >= 240
                || (a == 100 && (64..128).contains(&b))
                || (a == 198 && (18..20).contains(&b)))
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return public_address(IpAddr::V4(v4));
            }
            let first = v6.segments()[0];
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (first & 0xfe00) == 0xfc00
                || (first & 0xffc0) == 0xfe80
                || first == 0x2001 && v6.segments()[1] == 0x0db8)
        }
    }
}

fn checked_address(url: &Url) -> Result<SocketAddr, SourceProblem> {
    let host = url.host_str().ok_or(SourceProblem::InvalidAddress)?;
    let port = url.port_or_known_default().unwrap_or(443);
    let addresses: Vec<SocketAddr> = (host, port)
        .to_socket_addrs()
        .map_err(|_| SourceProblem::NotFound)?
        .collect();
    if addresses.is_empty()
        || addresses
            .iter()
            .any(|address| !public_address(address.ip()))
    {
        return Err(SourceProblem::NotPublic);
    }
    Ok(addresses[0])
}

/// Fetches one page over a connection pinned to its checked public address.
pub(crate) fn fetch_page(url: &Url) -> Result<SourcePage, SourceProblem> {
    let mut current = url.clone();
    for _ in 0..=MAX_REDIRECTS {
        let address = checked_address(&current)?;
        let host = current.host_str().unwrap_or_default().to_owned();
        let client = reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .referer(false)
            .resolve(&host, address)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(20))
            .user_agent("Mozilla/5.0 (compatible; Arrumo)")
            .build()
            .map_err(|_| SourceProblem::Unreachable)?;
        let response = client
            .get(current.clone())
            .header(
                reqwest::header::ACCEPT,
                "text/html,application/xhtml+xml,text/plain;q=0.8",
            )
            .send()
            .map_err(|error| {
                if error.is_timeout() {
                    SourceProblem::TimedOut
                } else {
                    SourceProblem::Unreachable
                }
            })?;
        if response.status().is_redirection() {
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or(SourceProblem::HttpError)?;
            current = source_url(
                current
                    .join(location)
                    .map_err(|_| SourceProblem::InvalidAddress)?
                    .as_str(),
            )?;
            continue;
        }
        if !response.status().is_success() {
            return Err(SourceProblem::HttpError);
        }
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("text/html")
            .to_ascii_lowercase();
        let plain = content_type.starts_with("text/plain");
        if !(plain
            || content_type.starts_with("text/html")
            || content_type.starts_with("application/xhtml"))
        {
            return Err(SourceProblem::NotAPage);
        }
        let mut bytes = Vec::new();
        response
            .take(MAX_PAGE_BYTES)
            .read_to_end(&mut bytes)
            .map_err(|_| SourceProblem::Unreachable)?;
        let body = String::from_utf8_lossy(&bytes);
        let mut page = if plain {
            SourcePage {
                url: String::new(),
                title: String::new(),
                description: String::new(),
                text: collapse(&body),
                images: Vec::new(),
                theme_color: None,
            }
        } else {
            let mut page = readable_page(&body);
            page.images = page_images(&body, &current);
            page
        };
        if page.text.is_empty() && page.description.is_empty() && page.images.is_empty() {
            return Err(SourceProblem::NoText);
        }
        page.url = current.to_string();
        page.text = page.text.chars().take(MAX_PAGE_CHARS).collect();
        return Ok(page);
    }
    Err(SourceProblem::TooManyRedirects)
}

const MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024;

/// Downloads one public image over a connection pinned to its checked address. Returns its bytes
/// and a file extension for its type.
pub(crate) fn fetch_image(url: &Url) -> Result<(Vec<u8>, &'static str), SourceProblem> {
    let mut current = url.clone();
    for _ in 0..=MAX_REDIRECTS {
        let address = checked_address(&current)?;
        let host = current.host_str().unwrap_or_default().to_owned();
        let client = reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .referer(false)
            .resolve(&host, address)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (compatible; Arrumo)")
            .build()
            .map_err(|_| SourceProblem::Unreachable)?;
        let response = client
            .get(current.clone())
            .header(
                reqwest::header::ACCEPT,
                "image/png,image/jpeg,image/webp,image/gif;q=0.8",
            )
            .send()
            .map_err(|error| {
                if error.is_timeout() {
                    SourceProblem::TimedOut
                } else {
                    SourceProblem::Unreachable
                }
            })?;
        if response.status().is_redirection() {
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or(SourceProblem::HttpError)?;
            current = source_url(
                current
                    .join(location)
                    .map_err(|_| SourceProblem::InvalidAddress)?
                    .as_str(),
            )?;
            continue;
        }
        if !response.status().is_success() {
            return Err(SourceProblem::HttpError);
        }
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_ascii_lowercase();
        let extension = match content_type.split(';').next().unwrap_or("").trim() {
            "image/png" => "png",
            "image/jpeg" | "image/jpg" => "jpg",
            "image/webp" => "webp",
            "image/gif" => "gif",
            _ => return Err(SourceProblem::NotAPage),
        };
        let mut bytes = Vec::new();
        response
            .take(MAX_IMAGE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| SourceProblem::Unreachable)?;
        if bytes.is_empty() || bytes.len() as u64 > MAX_IMAGE_BYTES {
            return Err(SourceProblem::NotAPage);
        }
        return Ok((bytes, extension));
    }
    Err(SourceProblem::TooManyRedirects)
}

/// Extracts the title, description and visible text from HTML without running it.
pub(crate) fn readable_page(html: &str) -> SourcePage {
    let lower = html.to_ascii_lowercase();
    let title = between(html, &lower, "<title", "</title>")
        .map(|inner| inner.split_once('>').map_or(inner, |(_, text)| text))
        .map(|text| collapse(&decode(text)))
        .unwrap_or_default();
    let description = ["og:description", "description", "twitter:description"]
        .iter()
        .find_map(|name| meta_content(html, &lower, name))
        .map(|text| collapse(&decode(&text)))
        .unwrap_or_default();
    let mut text = String::with_capacity(html.len() / 4);
    let mut index = 0;
    while index < html.len() {
        let Some(open) = lower[index..].find('<').map(|offset| index + offset) else {
            text.push_str(&html[index..]);
            break;
        };
        text.push_str(&html[index..open]);
        let skipped = ["script", "style", "noscript", "svg", "template", "head"]
            .iter()
            .find(|tag| {
                lower[open + 1..].starts_with(*tag)
                    && lower[open + 1 + tag.len()..].starts_with(['>', ' ', '\n', '\t', '\r', '/'])
            })
            .and_then(|tag| {
                lower[open..]
                    .find(&format!("</{tag}"))
                    .map(|end| open + end)
            });
        let from = skipped.unwrap_or(open);
        index = lower[from..]
            .find('>')
            .map_or(html.len(), |close| from + close + 1);
        text.push(' ');
    }
    let theme_color = meta_content(html, &lower, "theme-color")
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| {
            value.len() == 7
                && value.starts_with('#')
                && value[1..].bytes().all(|b| b.is_ascii_hexdigit())
        });
    SourcePage {
        url: String::new(),
        title,
        description,
        text: collapse(&decode(&text)),
        images: Vec::new(),
        theme_color,
    }
}

fn between<'a>(html: &'a str, lower: &str, open: &str, close: &str) -> Option<&'a str> {
    let start = lower.find(open)? + open.len();
    let end = lower[start..].find(close)? + start;
    html.get(start..end)
}

fn meta_content(html: &str, lower: &str, name: &str) -> Option<String> {
    let mut from = 0;
    while let Some(offset) = lower[from..].find("<meta") {
        let start = from + offset;
        let end = lower[start..]
            .find('>')
            .map_or(lower.len(), |close| start + close);
        let tag = &lower[start..end];
        if tag.contains(&format!("\"{name}\"")) || tag.contains(&format!("'{name}'")) {
            let original = &html[start..end];
            let content = tag.find("content=")? + "content=".len();
            let quote = original[content..].chars().next()?;
            if quote == '"' || quote == '\'' {
                let value = &original[content + 1..];
                return value.find(quote).map(|close| value[..close].to_owned());
            }
        }
        from = end;
    }
    None
}

fn decode(text: &str) -> String {
    text.replace("&nbsp;", " ")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_addresses_become_https_and_credentials_are_refused() {
        assert_eq!(
            source_url("example.com/about").unwrap().as_str(),
            "https://example.com/about"
        );
        assert!(source_url("ftp://example.com").is_err());
        assert!(source_url("https://user:pass@example.com").is_err());
        assert!(source_url("not a url").is_err());
    }

    #[test]
    fn only_public_addresses_are_contacted() {
        for private in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "::1",
            "fd00::1",
            "fe80::1",
            "::ffff:127.0.0.1",
        ] {
            assert!(!public_address(private.parse().unwrap()), "{private}");
        }
        for public in ["93.184.216.34", "2606:4700::1111"] {
            assert!(public_address(public.parse().unwrap()), "{public}");
        }
        assert!(checked_address(&Url::parse("http://localhost:8080/").unwrap()).is_err());
    }

    #[test]
    fn pages_keep_title_description_and_visible_text_only() {
        let page = readable_page(
            r#"<html><head><title>Ana &amp; Co</title>
            <meta property="og:description" content="Videos about coding agents">
            <script>steal()</script><style>p{}</style></head>
            <body><h1>Hello</h1><p>I review   AI tools.</p><noscript>enable js</noscript><svg><text>x</text></svg></body></html>"#,
        );
        assert_eq!(page.title, "Ana & Co");
        assert_eq!(page.description, "Videos about coding agents");
        assert_eq!(page.text, "Hello I review AI tools.");
    }

    #[test]
    fn pages_list_their_images_and_theme_colour() {
        let html = r##"<html><head><meta property="og:image" content="/share.png">
            <meta name="theme-color" content="#1E2A78"><link rel="icon" href="https://cdn.example.com/icon.png">
            </head><body><img src="/a.jpg" alt="Our studio"><img src="data:image/png;base64,AAA">
            <img src="http://insecure.example.com/b.png"><script>"i.ytimg.com/vi/dQw4w9WgXcQ/hq720.jpg"</script></body></html>"##;
        let base = Url::parse("https://example.com/about").unwrap();
        let images = page_images(html, &base);
        let urls: Vec<(&str, &str)> = images
            .iter()
            .map(|image| (image.url.as_str(), image.role))
            .collect();
        assert_eq!(
            urls,
            [
                ("https://example.com/share.png", "social"),
                ("https://cdn.example.com/icon.png", "icon"),
                (
                    "https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg",
                    "video_thumbnail"
                ),
                ("https://example.com/a.jpg", "image"),
            ]
        );
        assert_eq!(images[3].alt, "Our studio");
        assert_eq!(readable_page(html).theme_color.as_deref(), Some("#1e2a78"));
    }
}

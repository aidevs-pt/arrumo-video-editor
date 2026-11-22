//! Reference media for generation providers: workspace assets read by ID, checked by their bytes,
//! and uploaded through a provider's presigned-upload flow described in its SDK descriptor.

use std::{
    io::{self, Read},
    path::Path,
    time::Duration,
};

use reqwest::{
    blocking::Client,
    header::{HeaderMap, HeaderName, HeaderValue},
};
use serde::Deserialize;
use serde_json::Value;

use crate::WorkspaceStore;

pub(crate) const MAX_REFERENCE_IMAGES: usize = 9;
pub(crate) const MAX_REFERENCE_VIDEOS: usize = 3;
const MAX_IMAGE_BYTES: u64 = 20 * 1024 * 1024;
const MAX_VIDEO_BYTES: u64 = 200 * 1024 * 1024;
const MAX_UPLOAD_ANSWER_BYTES: u64 = 64 * 1024;
const MAX_UPLOAD_HEADERS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MediaKind {
    Image,
    Video,
}

/// A workspace image or video the creator chose to send to a provider.
pub(crate) struct ReferenceMedia {
    pub kind: MediaKind,
    pub mime: &'static str,
    pub bytes: Vec<u8>,
}

/// The content type of an image or MP4 file, from its first bytes.
pub(crate) fn media_mime(bytes: &[u8]) -> Option<(MediaKind, &'static str)> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some((MediaKind::Image, "image/png"))
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some((MediaKind::Image, "image/jpeg"))
    } else if bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some((MediaKind::Image, "image/webp"))
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some((MediaKind::Image, "image/gif"))
    } else if bytes.len() > 12 && &bytes[4..8] == b"ftyp" && &bytes[8..10] != b"qt" {
        Some((MediaKind::Video, "video/mp4"))
    } else {
        None
    }
}

impl WorkspaceStore {
    /// Reads the workspace's own images and MP4 videos for a provider request, in the given order.
    pub(crate) fn reference_media(
        &self,
        database: &Path,
        workspace_id: i64,
        asset_ids: &[i64],
    ) -> io::Result<Vec<ReferenceMedia>> {
        if asset_ids.len() > MAX_REFERENCE_IMAGES + MAX_REFERENCE_VIDEOS {
            return Err(io::Error::other("Too many reference files"));
        }
        let root = crate::asset_root(database);
        let mut media = Vec::new();
        for (index, id) in asset_ids.iter().enumerate() {
            if asset_ids[..index].contains(id) {
                return Err(io::Error::other("A reference file is listed twice"));
            }
            let kind: Option<String> = self
                .0
                .query_row(
                    "SELECT kind FROM workspace_assets WHERE workspace_id = ?1 AND id = ?2 AND superseded = 0",
                    [workspace_id, *id],
                    |row| row.get(0),
                )
                .ok();
            let limit = match kind.as_deref() {
                Some("image") => MAX_IMAGE_BYTES,
                Some("video") => MAX_VIDEO_BYTES,
                Some(_) => return Err(io::Error::other("References must be images or videos")),
                None => {
                    return Err(io::Error::other(
                        "A reference file is not in this workspace",
                    ));
                }
            };
            let mut bytes = Vec::new();
            std::fs::File::open(crate::asset_path(&root, workspace_id, *id))?
                .take(limit + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() as u64 > limit {
                return Err(io::Error::other("A reference file is too large"));
            }
            let (kind, mime) = media_mime(&bytes).ok_or_else(|| {
                io::Error::other("References must be PNG, JPEG, WebP or GIF images, or MP4 videos")
            })?;
            media.push(ReferenceMedia { kind, mime, bytes });
        }
        let images = media
            .iter()
            .filter(|item| item.kind == MediaKind::Image)
            .count();
        if images > MAX_REFERENCE_IMAGES || media.len() - images > MAX_REFERENCE_VIDEOS {
            return Err(io::Error::other(
                "Send at most 9 reference images and 3 reference videos",
            ));
        }
        Ok(media)
    }
}

/// How a provider takes uploads: a credentialed request on its own host returns a presigned
/// address to PUT the file to, and the public address to pass in the generation request.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct UploadDescriptor {
    endpoint: String,
    /// The JSON body; the string `{contentType}` is filled in.
    request: Value,
    upload_url: String,
    public_url: String,
    /// JSON pointer to an object of headers the PUT must carry. Only `content-type` and
    /// `x-amz-*` headers are sent.
    #[serde(default)]
    headers: Option<String>,
}

fn https_host(url: &str) -> Option<&str> {
    let rest = url.strip_prefix("https://")?;
    if url.contains(['#', '@', ' ', '\\']) || url.contains("/../") {
        return None;
    }
    rest.split(['/', '?'])
        .next()
        .filter(|host| !host.is_empty())
}

impl UploadDescriptor {
    pub(crate) fn valid(&self, endpoint: &str) -> bool {
        let pointer = |value: &str| value.starts_with('/') && value.len() <= 128;
        https_host(&self.endpoint).is_some()
            && https_host(&self.endpoint) == https_host(endpoint)
            && !self.endpoint.contains('?')
            && pointer(&self.upload_url)
            && pointer(&self.public_url)
            && self.headers.as_deref().is_none_or(pointer)
            && serde_json::to_vec(&self.request).is_ok_and(|body| body.len() <= 2048)
    }

    /// Uploads `media` and returns its public HTTPS address. The credential goes only to the
    /// descriptor's endpoint; the presigned PUT gets the file and the allowed headers alone.
    pub(crate) fn upload(
        &self,
        credential: (&HeaderName, &HeaderValue),
        media: &ReferenceMedia,
    ) -> io::Result<String> {
        let client = Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(300))
            .build()
            .map_err(|_| io::Error::other("Could not initialize the upload transport"))?;
        let body = fill_content_type(&self.request, media.mime);
        let answer = client
            .post(&self.endpoint)
            .header(credential.0.clone(), credential.1.clone())
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(
                serde_json::to_vec(&body)
                    .map_err(|_| io::Error::other("Could not encode the upload request"))?,
            )
            .send()
            .map_err(|_| io::Error::other("The provider's upload request failed"))?;
        if !answer.status().is_success() {
            return Err(io::Error::other(format!(
                "The provider refused the upload with HTTP {}",
                answer.status().as_u16()
            )));
        }
        let mut text = Vec::new();
        answer
            .take(MAX_UPLOAD_ANSWER_BYTES + 1)
            .read_to_end(&mut text)
            .map_err(|_| io::Error::other("Could not read the upload answer"))?;
        if text.len() as u64 > MAX_UPLOAD_ANSWER_BYTES {
            return Err(io::Error::other("The upload answer is too large"));
        }
        let answer: Value = serde_json::from_slice(&text)
            .map_err(|_| io::Error::other("The upload answer is not JSON"))?;
        let string = |pointer: &str| answer.pointer(pointer).and_then(Value::as_str);
        let upload_url = string(&self.upload_url)
            .filter(|url| https_host(url).is_some())
            .ok_or_else(|| io::Error::other("The provider gave no HTTPS upload address"))?;
        let public_url = string(&self.public_url)
            .filter(|url| https_host(url).is_some())
            .ok_or_else(|| io::Error::other("The provider gave no HTTPS address for the upload"))?
            .to_owned();
        let headers = upload_headers(
            self.headers
                .as_deref()
                .and_then(|pointer| answer.pointer(pointer)),
            media.mime,
        )?;
        let put = client
            .put(upload_url)
            .headers(headers)
            .body(media.bytes.clone())
            .send()
            .map_err(|_| io::Error::other("The reference file could not be uploaded"))?;
        if !put.status().is_success() {
            return Err(io::Error::other(format!(
                "The reference upload returned HTTP {}",
                put.status().as_u16()
            )));
        }
        Ok(public_url)
    }
}

fn fill_content_type(template: &Value, mime: &str) -> Value {
    match template {
        Value::String(text) if text == "{contentType}" => Value::String(mime.to_owned()),
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| fill_content_type(item, mime))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, item)| (key.clone(), fill_content_type(item, mime)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// The presigned PUT's headers: the provider's `content-type` and `x-amz-*` values, capped,
/// with the file's own content type when the provider names none.
fn upload_headers(given: Option<&Value>, mime: &'static str) -> io::Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    if let Some(given) = given {
        let object = given
            .as_object()
            .ok_or_else(|| io::Error::other("The upload headers are not an object"))?;
        if object.len() > MAX_UPLOAD_HEADERS {
            return Err(io::Error::other(
                "The provider asked for too many upload headers",
            ));
        }
        for (name, value) in object {
            let name = name.to_ascii_lowercase();
            if name != "content-type" && !name.starts_with("x-amz-") {
                continue;
            }
            let value = value
                .as_str()
                .filter(|value| value.len() <= 256)
                .ok_or_else(|| io::Error::other("An upload header is invalid"))?;
            headers.insert(
                HeaderName::from_bytes(name.as_bytes())
                    .map_err(|_| io::Error::other("An upload header is invalid"))?,
                HeaderValue::from_str(value)
                    .map_err(|_| io::Error::other("An upload header is invalid"))?,
            );
        }
    }
    if !headers.contains_key(reqwest::header::CONTENT_TYPE) {
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            HeaderValue::from_static(mime),
        );
    }
    Ok(headers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_types_come_from_the_bytes() {
        assert_eq!(
            media_mime(b"\x89PNG\r\n\x1a\nrest").map(|m| m.1),
            Some("image/png")
        );
        assert_eq!(
            media_mime(b"\0\0\0\x18ftypisom\0\0\0\0").map(|m| m.0),
            Some(MediaKind::Video)
        );
        assert!(media_mime(b"\0\0\0\x14ftypqt  \0\0\0\0").is_none());
        assert!(media_mime(b"<svg></svg>").is_none());
    }

    #[test]
    fn upload_headers_keep_only_content_type_and_storage_tags() {
        let given = serde_json::json!({
            "Content-Type": "image/png",
            "x-amz-tagging": "retention=temporary",
            "Authorization": "Key leaked",
            "Host": "evil.example"
        });
        let headers = upload_headers(Some(&given), "image/png").unwrap();
        assert_eq!(headers.len(), 2);
        assert!(headers.get("authorization").is_none() && headers.get("host").is_none());
        let too_many: serde_json::Map<String, Value> = (0..9)
            .map(|index| (format!("x-amz-{index}"), Value::from("v")))
            .collect();
        assert!(upload_headers(Some(&Value::Object(too_many)), "image/png").is_err());
        assert_eq!(
            upload_headers(None, "video/mp4")
                .unwrap()
                .get("content-type")
                .unwrap(),
            "video/mp4"
        );
    }

    #[test]
    fn uploads_stay_on_the_provider_host() {
        let descriptor: UploadDescriptor = serde_json::from_value(serde_json::json!({
            "endpoint": "https://api.example.com/files/generate-upload-url",
            "request": {"content_type": "{contentType}"},
            "uploadUrl": "/upload_url",
            "publicUrl": "/public_url",
            "headers": "/upload_headers"
        }))
        .unwrap();
        assert!(descriptor.valid("https://api.example.com/model/text-to-video"));
        assert!(!descriptor.valid("https://other.example.com/model"));
        assert_eq!(
            fill_content_type(&descriptor.request, "video/mp4"),
            serde_json::json!({"content_type": "video/mp4"})
        );
        assert!(https_host("https://bucket.s3.amazonaws.com/a?X-Amz-Signature=1").is_some());
        assert!(https_host("http://bucket.example/a").is_none());
        assert!(https_host("https://user@bucket.example/a").is_none());
    }

    #[test]
    fn references_must_be_this_workspace_s_media() {
        let store = WorkspaceStore::open(":memory:").unwrap();
        let workspace = store.create("Studio").unwrap().id;
        assert!(
            store
                .reference_media(Path::new(":memory:"), workspace, &[42])
                .is_err()
        );
        assert!(
            store
                .reference_media(Path::new(":memory:"), workspace, &[1, 1])
                .is_err()
        );
        assert!(
            store
                .reference_media(Path::new(":memory:"), workspace, &[])
                .unwrap()
                .is_empty()
        );
    }
}

/// A provider descriptor's line in each app language; `en` is used when one is missing.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Localized {
    pub en: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub es: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pt: Option<String>,
}

impl Localized {
    pub(crate) fn valid(&self, most: usize) -> bool {
        [Some(&self.en), self.es.as_ref(), self.pt.as_ref()]
            .into_iter()
            .flatten()
            .all(|text| (1..=most).contains(&text.chars().count()) && !text.contains(['\0', '\n']))
    }
}

use std::{
    collections::HashMap,
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
};

use http_range::HttpRange;
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};
use uuid::Uuid;

#[derive(Default)]
pub struct DeliveryStream(Mutex<Option<StreamServer>>);

struct StreamServer {
    origin: String,
    files: Arc<Mutex<HashMap<String, StreamFile>>>,
}

#[derive(Clone, PartialEq, Eq)]
struct StreamFile {
    path: PathBuf,
    content_type: &'static str,
}

impl DeliveryStream {
    pub fn open(&self, path: PathBuf) -> io::Result<String> {
        self.open_media(path, "video/mp4")
    }

    pub fn open_media(&self, path: PathBuf, content_type: &'static str) -> io::Result<String> {
        let mut server = self
            .0
            .lock()
            .map_err(|_| io::Error::other("Media stream unavailable"))?;
        if server.is_none() {
            *server = Some(StreamServer::start()?);
        }
        let server = server.as_ref().unwrap();
        let mut files = server
            .files
            .lock()
            .map_err(|_| io::Error::other("Media stream unavailable"))?;
        let media = StreamFile { path, content_type };
        let token = if let Some((token, _)) = files.iter().find(|(_, file)| **file == media) {
            token.clone()
        } else {
            let token = Uuid::new_v4().to_string();
            files.insert(token.clone(), media);
            token
        };
        Ok(format!("{}/{token}", server.origin))
    }
}

impl StreamServer {
    fn start() -> io::Result<Self> {
        let server = Server::http(("127.0.0.1", 0)).map_err(io::Error::other)?;
        let origin = format!("http://{}", server.server_addr());
        let files = Arc::new(Mutex::new(HashMap::new()));
        let access = Arc::clone(&files);
        thread::Builder::new()
            .name("arrumo-delivery-stream".into())
            .spawn(move || {
                for request in server.incoming_requests() {
                    let file = access.lock().ok().and_then(|files| {
                        files.get(request.url().trim_start_matches('/')).cloned()
                    });
                    if let Some(file) = file {
                        // Drop the registry lock before streaming. Each active video has its own request.
                        thread::spawn(move || respond(request, file));
                    } else {
                        let _ = request.respond(Response::empty(StatusCode(404)));
                    }
                }
            })?;
        Ok(Self { origin, files })
    }
}

fn respond(request: Request, StreamFile { path, content_type }: StreamFile) {
    if !matches!(request.method(), Method::Get | Method::Head) {
        let _ = request.respond(Response::empty(StatusCode(405)));
        return;
    }
    let file = (|| -> io::Result<File> {
        if !path.symlink_metadata()?.is_file() || path.canonicalize()? != path {
            return Err(io::Error::other("Delivery file unavailable"));
        }
        File::open(&path)
    })();
    let Ok(mut file) = file else {
        let _ = request.respond(Response::empty(StatusCode(404)));
        return;
    };
    let Ok(metadata) = file.metadata() else {
        let _ = request.respond(Response::empty(StatusCode(404)));
        return;
    };
    let size = metadata.len();
    let range = request
        .headers()
        .iter()
        .find(|header| header.field.equiv("Range"));
    let (offset, length, status) = match range {
        Some(header) => match HttpRange::parse(header.value.as_str(), size).as_deref() {
            Ok([range]) if range.length > 0 => (range.start, range.length, 206),
            _ => {
                let _ = request.respond(
                    Response::empty(StatusCode(416))
                        .with_header(header_value("Content-Range", format!("bytes */{size}"))),
                );
                return;
            }
        },
        None => (0, size, 200),
    };
    let (Ok(length_usize), Ok(_)) = (usize::try_from(length), file.seek(SeekFrom::Start(offset)))
    else {
        let _ = request.respond(Response::empty(StatusCode(500)));
        return;
    };
    let mut response = Response::new(
        StatusCode(status),
        vec![
            header_value("Content-Type", content_type),
            header_value("Accept-Ranges", "bytes"),
            header_value("Cache-Control", "no-store"),
        ],
        file.take(length),
        Some(length_usize),
        None,
    );
    if status == 206 {
        response.add_header(header_value(
            "Content-Range",
            format!("bytes {offset}-{}/{size}", offset + length - 1),
        ));
    }
    let _ = request.respond(response);
}

fn header_value(name: &str, value: impl AsRef<[u8]>) -> Header {
    Header::from_bytes(name, value.as_ref()).expect("fixed media header and ASCII value")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, net::TcpStream, time::Duration};

    #[test]
    fn stream_preserves_media_type_and_serves_byte_ranges() {
        let path = std::env::temp_dir().join(format!("arrumo-stream-{}", Uuid::new_v4()));
        std::fs::write(&path, b"test media").unwrap();
        let url = DeliveryStream::default()
            .open_media(path.canonicalize().unwrap(), "audio/wav")
            .unwrap();
        let (host, token) = url
            .strip_prefix("http://")
            .unwrap()
            .split_once('/')
            .unwrap();
        let mut client = TcpStream::connect(host).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        write!(
            client,
            "GET /{token} HTTP/1.1\r\nHost: {host}\r\nRange: bytes=5-9\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        let mut response = String::new();
        let result = client.read_to_string(&mut response);
        std::fs::remove_file(path).unwrap();
        result.unwrap();
        let (headers, body) = response.split_once("\r\n\r\n").unwrap();
        assert!(headers.starts_with("HTTP/1.1 206"));
        assert!(
            headers
                .to_ascii_lowercase()
                .contains("content-type: audio/wav")
        );
        assert_eq!(body, "media");
    }
}

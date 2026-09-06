#![allow(dead_code)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

const IO_TIMEOUT: Duration = Duration::from_secs(5);

pub struct ResponseSpec {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub delay: Duration,
}

pub struct RecordedRequest {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

pub struct FixtureServer {
    authority: SocketAddr,
    requests: Receiver<RecordedRequest>,
}

impl ResponseSpec {
    pub fn text(status: u16, body: impl Into<Vec<u8>>) -> Self {
        let body = body.into();
        Self {
            status,
            headers: vec![("Content-Length".into(), body.len().to_string())],
            body,
            delay: Duration::ZERO,
        }
    }

    pub fn redirect(location: &str) -> Self {
        Self {
            status: 302,
            headers: vec![
                ("Location".into(), location.into()),
                ("Content-Length".into(), "0".into()),
            ],
            body: Vec::new(),
            delay: Duration::ZERO,
        }
    }

    pub fn chunked(status: u16, body: impl AsRef<[u8]>) -> Self {
        let body = body.as_ref();
        let mut framed = format!("{:X}\r\n", body.len()).into_bytes();
        framed.extend_from_slice(body);
        framed.extend_from_slice(b"\r\n0\r\n\r\n");
        Self {
            status,
            headers: vec![
                ("Transfer-Encoding".into(), "chunked".into()),
                ("Connection".into(), "close".into()),
            ],
            body: framed,
            delay: Duration::ZERO,
        }
    }
}

impl FixtureServer {
    pub fn spawn(responses: Vec<ResponseSpec>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback fixture server");
        let authority = listener.local_addr().expect("read fixture server address");
        let (sender, requests) = mpsc::channel();

        thread::spawn(move || {
            for response in responses {
                let (mut stream, _) = listener.accept().expect("accept fixture request");
                let request = read_request(&mut stream);
                sender.send(request).expect("record fixture request");
                send_response(&mut stream, response);
            }
        });

        Self {
            authority,
            requests,
        }
    }

    pub fn url(&self) -> String {
        format!("http://{}", self.authority())
    }

    pub fn authority(&self) -> String {
        format!("127.0.0.1:{}", self.authority.port())
    }

    pub fn next_request(&self) -> RecordedRequest {
        self.requests
            .recv_timeout(IO_TIMEOUT)
            .expect("fixture server did not record a request")
    }
}

pub fn send_test_request(url: &str, method: &str, path: &str, body: &[u8]) -> u16 {
    let authority = url
        .strip_prefix("http://")
        .expect("fixture URL must use http://");
    let mut stream = TcpStream::connect(authority).expect("connect to fixture server");
    stream
        .set_read_timeout(Some(IO_TIMEOUT))
        .expect("set fixture client read timeout");
    stream
        .set_write_timeout(Some(IO_TIMEOUT))
        .expect("set fixture client write timeout");
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: {authority}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .expect("write fixture request headers");
    stream.write_all(body).expect("write fixture request body");
    stream.flush().expect("flush fixture request");

    let mut status_line = String::new();
    BufReader::new(stream)
        .read_line(&mut status_line)
        .expect("read fixture response status");
    status_line
        .split_whitespace()
        .nth(1)
        .expect("fixture response has a status code")
        .parse()
        .expect("fixture response status is numeric")
}

fn read_request(stream: &mut TcpStream) -> RecordedRequest {
    stream
        .set_read_timeout(Some(IO_TIMEOUT))
        .expect("set fixture server read timeout");
    let mut bytes = Vec::new();
    let mut chunk = [0; 1024];
    let header_end = loop {
        let read = stream.read(&mut chunk).expect("read fixture request");
        assert_ne!(read, 0, "fixture request ended before headers");
        bytes.extend_from_slice(&chunk[..read]);
        if let Some(position) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break position + 4;
        }
    };

    let header_text = String::from_utf8_lossy(&bytes[..header_end - 4]);
    let mut lines = header_text.split("\r\n");
    let request_line = lines.next().expect("fixture request has a request line");
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts
        .next()
        .expect("fixture request has a method")
        .to_owned();
    let path = request_parts
        .next()
        .expect("fixture request has a path")
        .to_owned();
    let headers: Vec<_> = lines
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.to_owned(), value.to_owned()))
        })
        .collect();
    let content_length = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("Content-Length"))
        .map(|(_, value)| value.trim().parse::<usize>().expect("valid Content-Length"))
        .unwrap_or(0);
    while bytes.len() - header_end < content_length {
        let read = stream.read(&mut chunk).expect("read fixture request body");
        assert_ne!(read, 0, "fixture request ended before its complete body");
        bytes.extend_from_slice(&chunk[..read]);
    }

    RecordedRequest {
        method,
        path,
        headers,
        body: bytes[header_end..header_end + content_length].to_vec(),
    }
}

fn send_response(stream: &mut TcpStream, response: ResponseSpec) {
    thread::sleep(response.delay);
    write!(
        stream,
        "HTTP/1.1 {} {}\r\n",
        response.status,
        status_reason(response.status)
    )
    .expect("write fixture response status");
    for (name, value) in response.headers {
        write!(stream, "{name}: {value}\r\n").expect("write fixture response header");
    }
    stream
        .write_all(b"\r\n")
        .expect("finish fixture response headers");
    stream
        .write_all(&response.body)
        .expect("write fixture response body");
    stream.flush().expect("flush fixture response");
}

fn status_reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Fixture Response",
    }
}

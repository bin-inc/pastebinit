use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use regex::Regex;
use reqwest::header::{ACCEPT_ENCODING, CONNECTION, CONTENT_TYPE};

use crate::input::python_rstrip;
use crate::posting::{EncodedBody, UploadPlan};
use crate::{AppError, AppResult, VERSION};

const RESULT_PAGE_ERROR: &str = "Unable to read or parse the result page, it could be a server timeout or a change server side, try with another pastebin.";

pub struct HttpResponse {
    pub final_url: String,
    pub body: Vec<u8>,
}

pub trait Transport {
    fn post(&self, plan: &UploadPlan) -> AppResult<HttpResponse>;
}

pub struct ReqwestTransport {
    client: reqwest::blocking::Client,
}

impl ReqwestTransport {
    pub fn new() -> AppResult<Self> {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(15))
            .user_agent(format!("Pastebinit v{VERSION}"))
            .build()
            .map_err(AppError::input)?;
        Ok(Self { client })
    }
}

impl Transport for ReqwestTransport {
    fn post(&self, plan: &UploadPlan) -> AppResult<HttpResponse> {
        if plan.url.starts_with("http://") {
            return post_http(plan);
        }
        let (body, content_type) = match &plan.body {
            EncodedBody::Form(body) => (body, "application/x-www-form-urlencoded"),
            EncodedBody::Json(body) => (body, "text/json"),
        };
        let mut response = self
            .client
            .post(&plan.url)
            .header(ACCEPT_ENCODING, "identity")
            .header(CONNECTION, "close")
            .header(CONTENT_TYPE, content_type)
            .body(body.clone())
            .send()
            .and_then(reqwest::blocking::Response::error_for_status)
            .map_err(AppError::input)?;
        let final_url = response.url().to_string();
        let mut response_body = Vec::new();
        response
            .read_to_end(&mut response_body)
            .map_err(AppError::input)?;
        Ok(HttpResponse {
            final_url,
            body: response_body,
        })
    }
}

fn post_http(plan: &UploadPlan) -> AppResult<HttpResponse> {
    let (body, content_type) = match &plan.body {
        EncodedBody::Form(body) => (body, "application/x-www-form-urlencoded"),
        EncodedBody::Json(body) => (body, "text/json"),
    };
    let mut url = plan.url.clone();
    let mut method = "POST";
    for _ in 0..10 {
        let parsed = reqwest::Url::parse(&url).map_err(AppError::input)?;
        let authority = parsed
            .socket_addrs(|| None)
            .map_err(AppError::input)?
            .into_iter()
            .next()
            .ok_or_else(|| AppError::input("no address found"))?;
        let mut stream =
            TcpStream::connect_timeout(&authority, Duration::from_secs(15)).map_err(|error| {
                match error.kind() {
                    std::io::ErrorKind::ConnectionRefused => {
                        AppError::input("<urlopen error [Errno 111] Connection refused>")
                    }
                    _ => AppError::input(error),
                }
            })?;
        stream
            .set_read_timeout(Some(Duration::from_secs(15)))
            .map_err(AppError::input)?;
        stream
            .set_write_timeout(Some(Duration::from_secs(15)))
            .map_err(AppError::input)?;
        let host = parsed
            .host_str()
            .ok_or_else(|| AppError::input("URL has no host"))?;
        let host = match parsed.port() {
            Some(port) => format!("{host}:{port}"),
            None => host.to_owned(),
        };
        let path = match parsed.query() {
            Some(query) => format!("{}?{query}", parsed.path()),
            None => parsed.path().to_owned(),
        };
        write!(
            stream,
            "{method} {path} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: Pastebinit v{VERSION}\r\nAccept-Encoding: identity\r\nConnection: close\r\n{body_headers}",
            body_headers = if method == "POST" {
                format!("Content-Type: {content_type}\r\nContent-Length: {}\r\n\r\n", body.len())
            } else {
                "\r\n".into()
            }
        )
        .map_err(AppError::input)?;
        if method == "POST" {
            stream.write_all(body).map_err(AppError::input)?;
        }
        stream.flush().map_err(AppError::input)?;

        let mut response = BufReader::new(stream);
        let mut status_line = String::new();
        response
            .read_line(&mut status_line)
            .map_err(AppError::input)?;
        let status = status_line
            .split_whitespace()
            .nth(1)
            .ok_or_else(|| AppError::input("invalid HTTP response"))?
            .parse::<u16>()
            .map_err(AppError::input)?;
        let mut location = None;
        let mut content_length = None;
        let mut chunked = false;
        loop {
            let mut line = String::new();
            response.read_line(&mut line).map_err(AppError::input)?;
            if line == "\r\n" || line.is_empty() {
                break;
            }
            if let Some((name, value)) = line.trim_end().split_once(':') {
                if name.eq_ignore_ascii_case("Location") {
                    location = Some(value.trim().to_owned());
                }
                if name.eq_ignore_ascii_case("Content-Length") {
                    content_length = value.trim().parse::<usize>().ok();
                }
                if name.eq_ignore_ascii_case("Transfer-Encoding")
                    && value
                        .split(',')
                        .any(|coding| coding.trim().eq_ignore_ascii_case("chunked"))
                {
                    chunked = true;
                }
            }
        }
        if (300..400).contains(&status) {
            let location = location.ok_or_else(|| AppError::input("redirect without Location"))?;
            url = parsed.join(&location).map_err(AppError::input)?.to_string();
            method = "GET";
            continue;
        }
        if status >= 400 {
            return Err(AppError::input(format!("HTTP Error {status}")));
        }
        let mut response_body = Vec::new();
        match (chunked, content_length) {
            (true, _) => read_chunked_body(&mut response, &mut response_body)?,
            (false, Some(length)) => {
                response
                    .take(length as u64)
                    .read_to_end(&mut response_body)
                    .map_err(AppError::input)?;
            }
            (false, None) => {
                response
                    .read_to_end(&mut response_body)
                    .map_err(AppError::input)?;
            }
        }
        return Ok(HttpResponse {
            final_url: url,
            body: response_body,
        });
    }
    Err(AppError::input("HTTP redirect limit exceeded"))
}

fn read_chunked_body(response: &mut BufReader<TcpStream>, body: &mut Vec<u8>) -> AppResult<()> {
    loop {
        let mut size_line = String::new();
        response
            .read_line(&mut size_line)
            .map_err(AppError::input)?;
        let size = size_line
            .trim_end_matches(['\r', '\n'])
            .split(';')
            .next()
            .unwrap_or_default();
        let size = usize::from_str_radix(size, 16).map_err(AppError::input)?;
        if size == 0 {
            loop {
                let mut trailer = String::new();
                response.read_line(&mut trailer).map_err(AppError::input)?;
                if trailer == "\r\n" || trailer.is_empty() {
                    return Ok(());
                }
            }
        }
        let start = body.len();
        body.resize(start + size, 0);
        response
            .read_exact(&mut body[start..])
            .map_err(AppError::input)?;
        let mut delimiter = [0; 2];
        response
            .read_exact(&mut delimiter)
            .map_err(AppError::input)?;
        if delimiter != *b"\r\n" {
            return Err(AppError::input("invalid HTTP chunk delimiter"));
        }
    }
}

pub fn submit_upload(transport: &dyn Transport, plan: &UploadPlan) -> AppResult<String> {
    let response = transport.post(plan)?;
    extract_paste_url(plan, &response.final_url, &response.body)
}

pub fn extract_paste_url(plan: &UploadPlan, final_url: &str, body: &[u8]) -> AppResult<String> {
    let Some(pattern) = &plan.response_pattern else {
        return Ok(final_url.to_owned());
    };
    let result = python_rstrip(std::str::from_utf8(body).map_err(|_| result_page_error())?);
    if pattern == "(.*)" {
        return Ok(result.to_owned());
    }

    let expression = Regex::new(pattern).map_err(|_| result_page_error())?;
    let captures = expression.captures(result).ok_or_else(result_page_error)?;
    let extracted = if expression.captures_len() > 1 {
        captures.get(1).ok_or_else(result_page_error)?.as_str()
    } else {
        let matched = captures.get(0).expect("successful captures has full match");
        &result[matched.end()..]
    };
    plan.target_url.as_deref().map_or_else(
        || Ok(format!("{}{extracted}", plan.base_url)),
        |target_url| python_percent_format(target_url, extracted),
    )
}

fn python_percent_format(template: &str, value: &str) -> AppResult<String> {
    let mut output = String::new();
    let characters: Vec<char> = template.chars().collect();
    let mut position = 0;
    let mut substitutions = 0;
    while let Some(&character) = characters.get(position) {
        position += 1;
        if character != '%' {
            output.push(character);
            continue;
        }
        if characters.get(position) == Some(&'%') {
            output.push('%');
            position += 1;
            continue;
        }

        let flags_start = position;
        while matches!(characters.get(position), Some('#' | '0' | '-' | ' ' | '+')) {
            position += 1;
        }
        let width_start = position;
        while matches!(characters.get(position), Some('0'..='9')) {
            position += 1;
        }
        let width = parse_format_number(&characters[width_start..position])?;
        let precision = if characters.get(position) == Some(&'.') {
            position += 1;
            let precision_start = position;
            while matches!(characters.get(position), Some('0'..='9')) {
                position += 1;
            }
            Some(parse_format_number(&characters[precision_start..position])?.unwrap_or(0))
        } else {
            None
        };
        if matches!(characters.get(position), Some('h' | 'l' | 'L')) {
            position += 1;
        }
        if characters.get(position) != Some(&'s') || substitutions != 0 {
            return Err(result_page_error());
        }
        position += 1;
        substitutions += 1;

        let value: String = value
            .chars()
            .take(precision.unwrap_or(usize::MAX))
            .collect();
        let padding = width.unwrap_or(0).saturating_sub(value.chars().count());
        if !characters[flags_start..width_start].contains(&'-') {
            output.extend(std::iter::repeat_n(' ', padding));
            output.push_str(&value);
        } else {
            output.push_str(&value);
            output.extend(std::iter::repeat_n(' ', padding));
        }
    }
    if substitutions == 1 {
        Ok(output)
    } else {
        Err(result_page_error())
    }
}

fn parse_format_number(characters: &[char]) -> AppResult<Option<usize>> {
    if characters.is_empty() {
        return Ok(None);
    }
    characters
        .iter()
        .collect::<String>()
        .parse()
        .map(Some)
        .map_err(|_| result_page_error())
}

fn result_page_error() -> AppError {
    AppError::input(RESULT_PAGE_ERROR)
}

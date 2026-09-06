use std::io::Read;
use std::time::Duration;

use regex::Regex;
use reqwest::header::CONTENT_TYPE;

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
        let (body, content_type) = match &plan.body {
            EncodedBody::Form(body) => (body, "application/x-www-form-urlencoded"),
            EncodedBody::Json(body) => (body, "text/json"),
        };
        let mut response = self
            .client
            .post(&plan.url)
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

pub fn submit_upload(transport: &dyn Transport, plan: &UploadPlan) -> AppResult<String> {
    let response = transport.post(plan)?;
    extract_paste_url(plan, &response.final_url, &response.body)
}

pub fn extract_paste_url(plan: &UploadPlan, final_url: &str, body: &[u8]) -> AppResult<String> {
    let Some(pattern) = &plan.response_pattern else {
        return Ok(final_url.to_owned());
    };
    let result = std::str::from_utf8(body)
        .map_err(|_| result_page_error())?
        .trim();
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
    match &plan.target_url {
        Some(target_url) if target_url.contains("%s") => {
            python_percent_format(target_url, extracted)
        }
        Some(target_url) => Ok(format!("{target_url}{extracted}")),
        None => Ok(format!("{}{extracted}", plan.base_url)),
    }
}

fn python_percent_format(template: &str, value: &str) -> AppResult<String> {
    let mut output = String::new();
    let mut characters = template.chars();
    let mut substitutions = 0;
    while let Some(character) = characters.next() {
        if character != '%' {
            output.push(character);
            continue;
        }
        match characters.next() {
            Some('%') => output.push('%'),
            Some('s') if substitutions == 0 => {
                output.push_str(value);
                substitutions += 1;
            }
            _ => return Err(result_page_error()),
        }
    }
    if substitutions == 1 {
        Ok(output)
    } else {
        Err(result_page_error())
    }
}

fn result_page_error() -> AppError {
    AppError::input(RESULT_PAGE_ERROR)
}

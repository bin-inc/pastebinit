use std::io::Read;

use crate::{AppError, AppResult};

pub struct InputDocument {
    pub display_name: String,
    pub content: String,
}

pub fn python_rstrip(value: &str) -> &str {
    value.trim_end_matches(|character: char| {
        character.is_whitespace() || matches!(character, '\u{001c}'..='\u{001f}')
    })
}

pub fn read_documents(files: &[String], stdin: &mut dyn Read) -> AppResult<Vec<InputDocument>> {
    let filenames: Vec<&str> = if files.is_empty() {
        vec!["-"]
    } else {
        files.iter().map(String::as_str).collect()
    };

    filenames
        .into_iter()
        .map(|filename| {
            let display_name = if filename == "-" {
                "STDIN".to_owned()
            } else {
                filename.to_owned()
            };
            let mut content = String::new();
            let read_result = if filename == "-" {
                stdin.read_to_string(&mut content)
            } else {
                std::fs::File::open(filename).and_then(|mut file| file.read_to_string(&mut content))
            };

            read_result
                .map_err(|_| AppError::input(format!("Error reading from: '{display_name}'")))?;

            let content = python_rstrip(&content).to_owned();
            if content.is_empty() {
                return Err(AppError::input(
                    "You are trying to send an empty document, exiting.",
                ));
            }

            Ok(InputDocument {
                display_name,
                content,
            })
        })
        .collect()
}

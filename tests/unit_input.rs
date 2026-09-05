use std::io::Cursor;

use pastebinit::input::{python_rstrip, read_documents};

#[test]
fn strips_the_same_trailing_whitespace_as_python_str_rstrip() {
    assert_eq!(python_rstrip("hello \n\t\u{001c}"), "hello");
    assert_eq!(python_rstrip("  hello  "), "  hello");
}

#[test]
fn reads_a_nonempty_file_and_records_its_filename() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("input.txt");
    std::fs::write(&path, "file content\n\t").unwrap();

    let mut stdin = Cursor::new(b"unused".as_slice());
    let documents = read_documents(&[path.display().to_string()], &mut stdin).unwrap();

    assert_eq!(documents.len(), 1);
    assert_eq!(documents[0].display_name, path.display().to_string());
    assert_eq!(documents[0].content, "file content");
}

#[test]
fn reads_dash_from_the_supplied_standard_input() {
    let mut stdin = Cursor::new("standard input\n\u{001c}".as_bytes());

    let documents = read_documents(&["-".into()], &mut stdin).unwrap();

    assert_eq!(documents.len(), 1);
    assert_eq!(documents[0].display_name, "STDIN");
    assert_eq!(documents[0].content, "standard input");
}

#[test]
fn defaults_to_standard_input_when_no_files_are_given() {
    let mut stdin = Cursor::new(b"default input\n".as_slice());

    let documents = read_documents(&[], &mut stdin).unwrap();

    assert_eq!(documents.len(), 1);
    assert_eq!(documents[0].display_name, "STDIN");
    assert_eq!(documents[0].content, "default input");
}

#[test]
fn rejects_input_that_is_empty_after_python_whitespace_trimming() {
    let mut stdin = Cursor::new(b" \n\t\x1c\x1d\x1e\x1f".as_slice());

    let error = match read_documents(&["-".into()], &mut stdin) {
        Err(error) => error,
        Ok(_) => panic!("empty input was accepted"),
    };

    assert_eq!(
        error.message(),
        "You are trying to send an empty document, exiting."
    );
}

#[test]
fn reports_a_missing_file_without_reading_later_inputs() {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("missing.txt");
    let mut stdin = PanicOnRead;

    let error = match read_documents(&[missing.display().to_string(), "-".into()], &mut stdin) {
        Err(error) => error,
        Ok(_) => panic!("missing file was accepted"),
    };

    assert_eq!(
        error.message(),
        format!("Error reading from: '{}'", missing.display())
    );
}

struct PanicOnRead;

impl std::io::Read for PanicOnRead {
    fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
        panic!("stdin was read after an earlier input failure");
    }
}

#[test]
fn preserves_source_order_when_reading_multiple_files() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("first.txt");
    let second = directory.path().join("second.txt");
    std::fs::write(&first, "first\n").unwrap();
    std::fs::write(&second, "second\n").unwrap();
    let mut stdin = Cursor::new(b"unused".as_slice());

    let documents = read_documents(
        &[first.display().to_string(), second.display().to_string()],
        &mut stdin,
    )
    .unwrap();

    assert_eq!(
        documents
            .iter()
            .map(|document| (document.display_name.clone(), document.content.clone()))
            .collect::<Vec<_>>(),
        vec![
            (first.display().to_string(), "first".into()),
            (second.display().to_string(), "second".into()),
        ]
    );
}

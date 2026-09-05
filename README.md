# pastebinit

`pastebinit` is an independently maintained native Rust implementation of the
pastebinit command-line pastebin client. It targets behavioral compatibility
with pastebinit 1.8.0.

The bundled reference fixture under `tests/reference/pastebinit-1.8.0/` is
used for differential compatibility testing.

## Security note

`-V` intentionally preserves pastebinit 1.8.0 compatibility by writing the
complete encoded request body to stderr. Consequently, `-V -p PASSWORD`
exposes the URL-encoded password in stderr. Do not combine `-V` with secrets
where stderr may be logged or visible to other users.

## License

GPL-2.0-or-later. See `LICENSE`.

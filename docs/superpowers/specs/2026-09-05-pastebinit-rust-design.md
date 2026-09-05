# pastebinit Rust Rewrite Design

## Status

Approved design for the first independently maintained release of
`bin-inc/pastebinit`.

## Purpose

Replace the Python implementation of pastebinit 1.8.0 with a native Rust
implementation while preserving the observable behavior, installed interface,
and configuration compatibility that Debian users and scripts rely on.

The source snapshot at
`/home/maw/src/bin-inc/origin-sources/pastebinit-1.8.0` is the behavioral
reference for this release.

## Project Identity And Licensing

- GitHub repository and upstream homepage: `https://github.com/bin-inc/pastebinit`
- Rust crate and Debian source package: `pastebinit`
- Debian binary package: `pastebinit`
- Installed commands: `pastebinit` and `pbput`
- License: GPL-2.0-or-later

The repository name identifies the familiar tool rather than its implementation
language. Project documentation and package metadata will say that this is an
independently maintained Rust implementation compatible with pastebinit 1.8.0.
Applicable upstream copyright notices and license material will be retained.

## Scope And Compatibility Contract

Release 1.9.0 reproduces pastebinit 1.8.0 behavior. It is not a redesigned
pastebin client. Any intentional deviation must be documented in release notes
and covered by an explicit compatibility test exception.

### Primary Client

The `pastebinit` command preserves:

- Command name, option names, accepted positional and `-i` file input, default
  values, help/version behavior, standard streams, and exit status, including
  status 2 for invalid command-line invocation and status 1 for operational
  failures.
- Input from stdin and one or more files, including trailing-whitespace removal,
  empty-content rejection, `-E` echo output, and fail-fast handling.
- `-l` listing behavior and `-V` diagnostic output.
- `-a`, `-t`, `-f`, `-P`, `-e`, `-u`, and `-p` option semantics.
- Default pastebin selection: `bpa.st` generally, `paste.debian.net` for Debian
  and Raspbian, `paste.centos.org` for Fedora/CentOS/RHEL/Rocky, and
  `paste.opensuse.org` for SUSE/SLES.
- HTTP/HTTPS POST behavior, the 15-second request timeout, redirects, standard
  form encoding, JSON request encoding, URL output, response regex extraction,
  `target_page`, `sizelimit`, and `user_length` handling.
- Failure handling for unknown pastebins, unreadable/empty input, invalid
  configuration, oversize input, transport failures, and unreadable or
  unparsable result pages.

The compatibility target includes the shipped pastebin definitions and support
for site-specific custom fields in configuration.

### Configuration

The implementation preserves `pastebin.d` INI syntax and its overlay semantics.
It searches the following locations in the reference order, with later files
overriding earlier definitions of the same pastebin basename:

1. `/usr/share/pastebin.d`
2. `/usr/local/share/pastebin.d`
3. Entries in `XDG_DATA_DIRS`, traversed in reverse order
4. Entries in `XDG_CONFIG_DIRS`, traversed in reverse order
5. `/etc/pastebin.d`
6. `/usr/local/etc/pastebin.d`
7. `${XDG_CONFIG_HOME:-~/.config}/pastebin.d`
8. `~/.pastebin.d`
9. The directory adjacent to the executable/source, with `pastebin.d` appended
   unless the path already ends in `pastebin.d`

The legacy XML preferences file remains supported in 1.9.0. The loader searches
reversed `XDG_CONFIG_DIRS`, then `/etc`, `/usr/local/etc`,
`${XDG_CONFIG_HOME:-~/.config}`, and `~/.pastebinit.xml`. It accepts the
reference values `pastebin`, `author`, `format`, `private`, and `expiry`.

`pastebinit.xml` has no upstream deprecation notice in the 1.8.0 source, so it
is a supported compatibility input rather than a deprecated feature. The
project may only deprecate it after a feature-complete modern replacement and a
separate documented release cycle.

### Auxiliary Command

The Debian 1.8.0 package installs `pbput` as its sole auxiliary executable. The
source script has `pbputs` and `pbget` code paths selected by its invocation
name, but `debian/install` installs only `utils/pbput`; it ships neither
`pbputs` nor `pbget` aliases. Exact package parity therefore requires only
`pbput`.

`pbput` reads stdin or archives a named file/directory, LZMA-compresses and
Base64-encodes it, then posts it through `pastebinit -b paste.ubuntu.com`.
It preserves its argument convention, output, cleanup-on-interruption behavior,
and the installed `pbput(1)` documentation. The historical manual page's
documentation of uninstalled `pbputs` and `pbget` invocation names remains
outside the 1.9.0 executable compatibility surface.

## Architecture

Use one Cargo package with a reusable library and two thin command binaries.
The library separates compatibility-sensitive behavior into focused modules:

- `cli`: parses the established primary-client options and renders compatible
  usage, version, and diagnostics.
- `configuration`: determines distribution defaults, discovers configuration
  paths, parses and overlays INI definitions, and applies legacy XML values.
- `input`: loads stdin/files, applies reference trimming/empty checks, and
  emits echo output.
- `posting`: resolves endpoints, builds parameters, enforces site constraints,
  performs HTTP requests, and derives resulting URLs.
- `helpers`: implements the `pbput` archive, LZMA, Base64, and posting workflow.
- `i18n`: loads translations under the `pastebinit` gettext domain.
- `platform`: isolates Debian paths and distribution detection from portable
  logic.

The binaries orchestrate these modules but contain no independently defined
compatibility policy. This keeps behavior individually testable and avoids
platform-specific assumptions spreading through the core implementation.

## Error Handling And Security

All expected operational errors return nonzero, write diagnostics to stderr,
and stop processing further input, matching the reference fail-fast behavior.
Normal URL output is written to stdout. Interruptions also exit nonzero after
cleaning temporary resources.

Verbose mode reproduces the reference diagnostic: it writes the POST endpoint
and encoded request parameters to stderr. This includes a supplied `password`
value. The behavior is retained for exact compatibility and documented as a
credential-exposure risk; users must not enable `-V` with credentials in an
untrusted logging environment.

The helper implementation uses safely created temporary files, cleans them on
normal exit and signals, and delegates archive creation to the system `tar`
command to match the installed reference helper.

## Testing Strategy

### Test Oracle

The Python reference executable from the 1.8.0 source tree is the differential
test oracle. Tests run it and the Rust executable in identical temporary
filesystem environments and compare observable results. External pastebin
services are not test dependencies.

A local fixture HTTP server records incoming requests and produces controlled
responses, including redirects, direct URLs, regex-extractable bodies, invalid
UTF-8, malformed replies, delayed replies, status failures, and closed
connections.

Differential assertions compare:

- Process exit status.
- Exact stdout and stderr bytes, except intentionally version-specific identity
  output that is asserted separately.
- HTTP method, URL, headers, content type, and raw, form, or JSON request body.
- Returned URL behavior, including redirect, `paste_regexp`, and `target_page`
  handling.
- Configuration resolution, precedence, and malformed/missing-file behavior.

### Unit Tests

Unit tests cover deterministic components without process or network setup:

- CLI defaults and option parsing.
- Distribution-default selection and configuration-path ordering.
- INI/XML parsing, invalid configuration cases, and option precedence.
- Request parameter construction, form/JSON encoding, size limits, and username
  truncation.
- Response regex extraction and target URL composition.
- `pbput` Base64/LZMA transforms, stdin/file/directory source selection, and
  propagation of `pastebinit` child-process failures.

### Integration And End-To-End Tests

Integration tests execute installed or built command binaries inside clean
temporary homes and XDG environments. Primary-client coverage includes stdin,
positional files, `-i`, multiple files, `-E`, `-l`, verbose mode, every optional
field, distribution defaults, custom configuration, form and JSON posts, and
all failure classes.

End-to-end differential tests exercise the same scenarios against the Python
reference and Rust implementation using the fixture server. Helper tests cover
stdin and file/directory uploads, LZMA/Base64 round trips, temporary-resource
cleanup, and `pastebinit` child-process failures.

## Debian Packaging

The Debian package is named `pastebinit`, installs both commands, manpages,
shipped `pastebin.d` definitions, and gettext catalogs at their conventional
paths. It is architecture-dependent because it contains Rust native binaries.

The package deliberately replaces the original package in place by using the
same Debian binary-package name and a higher version. It must not declare
`Conflicts` or `Replaces` against itself. Packaging uses standard Debian Rust
tooling appropriate to the target release, such as `dh-cargo`, and declares
runtime dependencies necessary for the installed `pbput` helper when those
capabilities are delegated to system tools.

Debian is the first supported environment. Portable core modules and
platform-isolated path/default behavior leave room for future packages without
making them a 1.9.0 delivery requirement.

## Versioning And Release Process

Project releases use Semantic Versioning. The first public candidate and stable
release are:

| Project tag | Debian package version | Purpose |
| --- | --- | --- |
| `v1.9.0-rc.1` | `1.9.0~rc.1-1` | Public release candidate |
| `v1.9.0` | `1.9.0-1` | First stable Rust replacement |

The Debian `~` mapping preserves ordering:

```text
1.8.0-1 < 1.9.0~rc.1-1 < 1.9.0-1
```

The `-1` suffix is a Debian revision and is not part of the public SemVer
version. Git tags, `pastebinit -v`, documentation, and release notes use
`1.9.0-rc.1` and `1.9.0`.

The release candidate satisfies the same automated compatibility and packaging
requirements as stable. Its external validation period is for discovering final
defects, not for deferring known compatibility gaps.

Every candidate and stable release includes a signed Git tag, source archive,
Debian source and binary packages, checksums, release notes describing
compatibility and intentional deviations, and SBOM/provenance artifacts when
the release pipeline supports them.

## Release Gates

`v1.9.0-rc.1` and `v1.9.0` may be published only when all of the following
pass in CI on supported Debian releases:

1. Formatting and linting.
2. Unit tests.
3. Integration tests.
4. Differential end-to-end compatibility tests.
5. Dependency security checks.
6. Debian source/binary package build.
7. Installation and command smoke tests from the produced Debian package.
8. License, copyright, and package metadata review.

Known behavioral deviations block the stable release unless explicitly accepted,
documented, and justified as a security or platform necessity.

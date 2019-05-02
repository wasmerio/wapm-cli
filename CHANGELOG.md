# Changelog

All PRs to the `wapm-cli` repository must add to this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2019-04-22
☄ First release of `wapm-cli` 🌌

## **[Unreleased]**

### Added
- Install packages with name and version e.g. `wapm install lua@0.1.2`.
- Fall back to default for `WASMER_DIR` env var if it doesn't exist
- Global install packages from the registry with `-g`/`--global` flag.
  - Packages are installed into `WASMER_DIR/globals`.
  - Packages are runnable from any directory that does not already have that package installed.
- Packages are runnable from any directory that does not already have that package installed.
- List subcommand (`wapm list`) to show packages and commands installed
- Enforce semantic version numbers on the package and dependencies.
- Allow ranges of semantic versions when declaring dependencies e.g. `"_/sqlite": "^0.1"`
- Uninstall a package with `wapm uninstall` and use the `-g` flag for global uninstall.
- Get the bin directory for wapm-run-scripts using `wapm bin` command.
- Add CI job for Windows
- Add CI integration tests
### Changed
- Refactored process for generating updates to manifest, regenerating the lockfile, and installing packages.
- Changed OpenSSL to statically link for Linux builds (because version 1.1 is not widely deployed yet)
### Fixed
- Fixed installing packages with http responses that are missing the gzip content encoding header.

[Unreleased]: https://github.com/wasmerio/wapm-cli/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/wasmerio/wapm-cli/releases/tag/v0.1.0

# Changelog

All PRs to the `wapm-cli` repository must add to this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2019-04-22
☄ First release of `wapm-cli` 🌌

## **[Unreleased]**

### Added
- Install packages with name and version e.g. `wapm install lua@0.1.2`.
### Changed
- Refactored process for generating updates to manifest, regenerating the lockfile, and installing packages.
### Fixed
- Fixed installing packages with http responses that are missing the gzip content encoding header.

[Unreleased]: https://github.com/wasmerio/wapm-cli/compare/0.1.0...HEAD
[0.1.0]: https://github.com/wasmerio/wapm-cli/releases/tag/v0.1.0

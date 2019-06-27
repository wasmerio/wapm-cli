# Changelog

All PRs to the `wapm-cli` repository must add to this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## **[Unreleased]**
### Added
- `--force-yes` flag to `wapm install` which accepts all prompts
- `--dry-run` flag to `wapm publish` which runs the publish logic without sending anything to the registry
- validation of the manifest on publish, all commands must reference valid modules
- wapm will now suggest a package to install that contains the desired command if the command is not found

### Changed
- Lockfile version 3 with package root directory added

## [0.3.1] - 2019-06-19
### Added
- Bug fix to stop wapm from entirely blocking consuming unsigned packages from producers for whom the consumer has a public key
- `keys generate` convenience subcommand
- Package filesystem support allowing filesystems to be bundled with wapm packages

## [0.3.0] - 2019-06-17
### Added
- Wasm contracts (experimental way of validating imports and exports)
- Package signing
  - Packages can be signed and verified with Minisign keys
  - `wapm keys` for relevant subcommands
- SQLite database as backing store for data like keys and contracts

## [0.2.0] - 2019-05-06
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
- Publish wapm package with license file
- Add CI job for Windows
- Add CI integration tests
### Changed
- Refactored process for generating updates to manifest, regenerating the lockfile, and installing packages.
- Changed OpenSSL to statically link for Linux builds (because version 1.1 is not widely deployed yet)
- Statically link LibSSL 1.1 on Linux
### Fixed
- Fixed installing packages with http responses that are missing the gzip content encoding header.

## [0.1.0] - 2019-04-22
☄ First release of `wapm-cli` 🌌

[Unreleased]: https://github.com/wasmerio/wapm-cli/compare/v0.3.1...HEAD
[0.3.1]: https://github.com/wasmerio/wapm-cli/releases/tag/v0.3.1
[0.3.0]: https://github.com/wasmerio/wapm-cli/releases/tag/v0.3.0
[0.2.0]: https://github.com/wasmerio/wapm-cli/releases/tag/v0.2.0
[0.1.0]: https://github.com/wasmerio/wapm-cli/releases/tag/v0.1.0

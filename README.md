<p align="center">
  <a href="https://wapm.dev" target="_blank" rel="noopener noreferrer">
    <img width="400" src="assets/logo.png" alt="Wapm logo">
  </a>
</p>

<p align="center">
  <a href="https://dev.azure.com/wasmerio/wapm-cli/_build?definitionId=2">
    <img src="https://dev.azure.com/wasmerio/wapm-cli/_apis/build/status/wasmerio.wapm-cli?branchName=master" alt="Build Status">
  </a>
  <a href="https://github.com/wasmerio/wasmer/blob/master/LICENSE">
    <img src="https://img.shields.io/github/license/wasmerio/wasmer.svg" alt="License">
  </a>
  <a href="https://spectrum.chat/wasmer">
    <img src="https://withspectrum.github.io/badge/badge.svg" alt="Join the Wasmer Community">
  </a>
</p>

# Introduction

The WebAssembly Package Manager CLI

```
wapm
```

## Commands

#### `wapm login`

Logins the user in the registry with the given credentials.

#### `wapm logout`

Logouts the user from the registry, resetting the token.

#### `wapm whoami`

Shows the current user logged in.

#### `wapm config set <key> <value>`

Sets a config `key` with the given `value`.

_Note: when setting the `registry.url`, the `registry.token` will automatically be resetted._

#### `wapm config get <key>`

Gets the config `key` contents.

#### `wapm search <query>`

Search for packages related to the `query`.

#### `wapm package`

One can bundle a project by running the package command in a directory with a `wapm.toml` or by passing the path to 
`wapm.toml` in a command-line flag. 

```
# In a directory with a manifest
> wapm package

# Or with a path to the manifest
> wapm package -m path/to/wapm.toml
```

Bundled assets are stored in a [custom section][1] of WebAssembly. Assets are archived and compressed before stored in 
the custom section. Runtimes like [Wasmer][2] can read the custom section and  expose the archived files via virtual 
filesystem. 

A header is written to the beginning of the custom section containing metadata about the compression and archive. This
information is useful to runtimes like Wasmer that unpack bundled assets. The header is a 4 bytes block and contains
the compression type and archive type.

The bundled files may be specified on the command line or in the manifest file:

```
# cli
> wapm package -a foo.txt:foo.txt,bar.txt:new_bar.txt,my_dir:my/dir
```

```toml
# wapm.toml
[fs]
"foo.txt" = "foo.txt"
"bar.txt" = "new_bar.txt"
"my_dir" = "my/dir"
```

#### `wapm run`

One can execute a package command with the `run` command. The command will be run with the wasmer runtime.

## Manifest (`wapm.toml`)

The manifest file describes how to bundle a wasm package. A simple example manifest with all required fields:
```toml
name = "app"
description = "My awesome app is awesome."
version = "0.1.0"
source = "app.wasm"
module = "app_bundle.wasm"
```

## Development

### Update GraphQL Schema

If the WAPM GraphQL server has been updated, update the GraphQL schema with:

```
graphql get-schema -e dev
```

_Note: You will need graphql-cli installed for it `npm install -g graphql-cli`._

[1]: https://webassembly.github.io/spec/core/appendix/custom.html
[2]: https://wasmer.io
<p align="center">
  <a href="https://wapm.dev" target="_blank" rel="noopener noreferrer">
    <img width="400" src="assets/logo.png" alt="Wapm logo">
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

#### `wapm run`

One can execute a package command with the `run` command. The command will be run with the wasmer runtime.

## Manifest (`wapm.toml`)

The manifest file describes how to describe a wasm package. The manifest is optional and should live in 
the root directory of a wapm project. A corresponding `wapm.lock` file is generated when running `wapm`
commands.

An example manifest:

```toml
[package]
name = "username/app"
description = "My awesome app is awesome."
version = "0.1.0"

[dependencies]
dep_name = "0.1.0"

[[module]]
name = "my_app"
source = "app.wasm"

[[command]]
name = "run"
module = "my_app"
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
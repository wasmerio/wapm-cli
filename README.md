# Wapm CLI

The WebAssembly Package Manager CLI

```
wapm
```

## Commands

## `wapm login`

Logins the user in the registry.

Example:

```
wapm login
```

## `wapm logout`

Logouts the user from the registry, resetting the token.

Example:

```
wapm login
```

## `wapm whoami`

Shows the current user logged in.

Example:

```
wapm whoami
```

### Config

Operate with the wapm configuration

### `wapm config set <key> <value>`

Sets a config `key` with the given `value`.

Example:

```bash
wapm config set registry.url https://registry.wasmer.io/
```

_Note: when setting the `registry.url`, the `registry.token` will automatically be resetted._

### `wapm config get <key>`

Gets the config `key` contents.

Example:

```bash
wapm config get registry.url
```

## Development

### Update GraphQL Schema

If the WAPM GraphQL server has been updated, update the GraphQL schema with:

```
graphql get-schema -e dev
```

_Note: You will need graphql-cli installed for it `npm install -g graphql-cli`._

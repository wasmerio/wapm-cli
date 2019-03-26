# Wapm CLI

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

## Development

### Update GraphQL Schema

If the WAPM GraphQL server has been updated, update the GraphQL schema with:

```
graphql get-schema -e dev
```

_Note: You will need graphql-cli installed for it `npm install -g graphql-cli`._

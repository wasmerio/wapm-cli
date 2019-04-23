<p align="center">
  <a href="https://wapm.io" target="_blank" rel="noopener noreferrer">
    <img height="110" src="assets/logo.png" alt="Wapm logo">
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

The WebAssembly Package Manager CLI. This tool enables installing, managing, and publishing wasm packages on the [wapm.io][wapmio] registry. 

## Get Started

Read the [`wapm-cli` user guide on `wapm.io`][guide] to get started using the tool and use the [`wapm-cli` reference][reference]
for information about the cli commands.

## Get Help

Join the discussion on [spectrum chat][spectrum] in the `wapm-cli` channel, or create a GitHub issue. We love to help!

## Contributing

See the [contributing guide][contributing] for instruction on contributing to `wapm-cli`.

## Development

### Update GraphQL Schema

If the WAPM GraphQL server has been updated, update the GraphQL schema with:

```
graphql get-schema -e prod
```

_Note: You will need graphql-cli installed for it `npm install -g graphql-cli`._

[contributing]: CONTRIBUTING.md
[guide]: https://wapm.io/help/guide
[reference]: https://wapm.io/help/reference
[spectrum]: https://spectrum.chat/wasmer
[wasmer]: https://wasmer.io
[wapmio]: https://wapm.io

#!/bin/sh

#alias wapm=target/debug/wapm
wapm config set registry.url "https://registry.wapm.dev"
yes 2> /dev/null | wapm install -g mark2/dog2@0.0.12
wapm run dog -- data
wapm install mark2/dog2@0.0.12
wapm run dog -- data
wapm uninstall mark2/dog2
wapm uninstall -g mark2/dog2
cp wapm_packages/mark2/dog2@0.0.12/dog.wasm .
echo '[package]\nname="test"\nversion="0.0.0"\ndescription="this is a test"\n[[module]]\nname="test"\nsource="dog.wasm"\n[[command]]\nname="test"\nmodule="test"\n[fs]\n"wapm_file"="src/bin"' > wapm.toml
wapm run test -- wapm_file/bin
rm dog.wasm

#!/bin/sh

alias wapm=target/debug/wapm
wapm config set registry.url "https://registry.wapm.dev"
yes | wapm install -g mark2/dog2@0.0.6
wapm run dog
wapm install mark2/dog2@0.0.6
wapm run dog
wapm uninstall mark2/dog2
wapm uninstall -g mark2/dog2
cp wapm_packages/mark2/dog2@0.0.6/dog.wasm .
echo '[package]\nname="test"\nversion="0.0.0"\ndescription="this is a test"\npkg-fs-mount-point="."\n[[module]]\nname="test"\nsource="dog.wasm"\n[[command]]\nname="test"\nmodule="test"\n[fs]\n"wapm_file"="src/bin/wapm.rs"' > wapm.toml
wapm run test
rm dog.wasm
rm -rf pkg_fs

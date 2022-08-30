#!/bin/sh
mkdir manifest_validation
chmod 777 manifest_validation
cd manifest_validation
set -x
export RUST_BACKTRACE=1
ln -sf `which wapm` wax
wapm config set registry.url "https://registry.wapm.dev"
echo '[package]\nname="test"\nversion="0.0.0"\ndescription="this is a test"\n[[command]]\nname="test"\nmodule="test-module"\n[fs]\n"wapm_file"="src/bin"' > wapm.toml
wapm publish --dry-run
# get a wasm module so we forget the abi field
wapm install mark2/dog2@0.0.13 --force-yes
cp wapm_packages/mark2/dog2@0.0.13/dog.wasm .
echo '[package]\nname="test"\nversion="0.0.0"\ndescription="this is a test"\n[[module]]\nname="test-module"\nsource="dog.wasm"\n[[command]]\nname="test"\nmodule="test-module"\n[fs]\n"wapm_file"="src/bin"' > wapm.toml
wapm publish --dry-run
echo '[package]\nname="test"\nversion="0.0.0"\ndescription="this is a test"\n[[module]]\nname="test-module"\nsource="dog.wasm"\nabi="wasi"\n[[command]]\nname="test"\nmodule="test-module"\n[fs]\n"wapm_file"="src/bin"' > wapm.toml
wapm publish --dry-run
rm dog.wasm
cd ..
rm -rf ./manifest_validation
#!/bin/sh
mkdir package_fs_mapping
chmod 777 package_fs_mapping
cd package_fs_mapping
set -x
export RUST_BACKTRACE=1
ln -sf `which wapm` wax
wapm config set registry.url "https://registry.wapm.dev"
wapm install -g mark2/dog2@0.0.13 --force-yes
wapm run dog -- data
wapm uninstall -g mark2/dog2
wapm install mark2/dog2@0.0.13
wapm run dog -- data
wapm uninstall mark2/dog2
cp wapm_packages/mark2/dog2@0.0.13/dog.wasm .
echo '[package]\nname="test"\nversion="0.0.0"\ndescription="this is a test"\n[[module]]\nname="test-module"\nsource="dog.wasm"\n[[command]]\nname="test"\nmodule="test-module"\n[fs]\n"wapm_file"="src/bin"' > wapm.toml
wapm run test -- wapm_file
rm dog.wasm
cd ..
rm -rf ./package_fs_mapping
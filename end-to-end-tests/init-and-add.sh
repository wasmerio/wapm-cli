#!/bin/sh
mkdir init_and_add
chmod 777 init_and_add
cd init_and_add
set -x
export RUST_BACKTRACE=1
ln -sf `which wapm` wax
wapm config set registry.url "https://registry.wapm.dev"

mkdir test-package
cd test-package
wapm config set registry.url "https://registry.wapm.dev"
wapm init -y
wapm add this-package-does-not-exist
wapm add mark2/python@0.0.4 mark2/dog2
wapm add lolcat@0.1.1
wapm remove lolcat
cd ..
rm -rf test-package
cd ..
rm -rf ./test
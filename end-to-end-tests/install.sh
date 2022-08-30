#!/bin/sh
mkdir install
chmod 777 install
cd install
set -x
export RUST_BACKTRACE=1
ln -sf `which wapm` wax
wapm config set registry.url "https://registry.wapm.dev"
wapm install namespace-example/cowsay@0.1.2
wapm install namespace-example/cowsay@0.1.2
wapm run cowsay "hello, world"
wapm list
wapm uninstall namespace-example/cowsay
wapm install namespace-example/cowsay@0.1.2
wapm uninstall namespace-example/cowsay
wapm uninstall namespace-example/cowsay
wapm install -g mark/rust-example@0.1.11
wapm run hq9+ -e "H"
wapm uninstall -g mark/rust-example
wapm install -g mark/wapm-override-test@0.1.0
wapm list -a
wapm run wapm-override-test
wapm install mark/wapm-override-test@0.2.0
wapm run wapm-override-test
wapm uninstall mark/wapm-override-test
wapm run wapm-override-test
wapm uninstall -g mark/wapm-override-test
wapm install namespace-example/cowsay@0.1.1 namespace-example/cowsay@0.1.2
cd ..
rm -rf ./install
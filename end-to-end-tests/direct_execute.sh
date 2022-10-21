#!/bin/sh

export RUST_BACKTRACE=1
ln -sf `which wapm` wax
wapm config set registry.url "https://registry.wapm.dev"

# echo "hello" | wapm execute base64
# ./wax echo "hello"
wapm install namespace-example/cowsay
./wax --emscripten cowsay "hello"
wapm uninstall namespace-example/cowsay
wapm install lolcat
wapm run lolcat -V
./wax lolcat -V
wapm uninstall lolcat
./wax lolcat -V
wapm list -a
rm -rf $(./wax --which lolcat)/wapm_packages/_/lolcat@0.1.1/*
./wax lolcat -V
./wax --offline lolcat -V
# WAPM_RUNTIME=echo ./wax ls | grep "\-\-command-name" || echo "Success: command-name not found"

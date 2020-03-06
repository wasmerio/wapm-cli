#!/bin/sh

export RUST_BACKTRACE=1
alias wapm=target/debug/wapm
alias wax=target/debug/wapm
wapm config set registry.url "https://registry.wapm.dev"

echo "hello" | wapm execute base64
wax echo "hello"
wax cowsay --emscripten "hello"
wapm install lolcat
wapm run lolcat -- -V
wax lolcat -- -V
wapm uninstall lolcat
wax lolcat -- -V
wapm list -a
rm -rf $(wax --which lolcat)/wapm_packages/_/lolcat@0.1.1/*
wax lolcat -- -V

#!/bin/sh

export RUST_BACKTRACE=1
alias wapm=target/debug/wapm
wapm config set registry.url "https://registry.wapm.dev"

echo "hello" | wapm execute base64
wapm execute echo "hello"
wapm execute cowsay --emscripten "hello"
wapm install lolcat
wapm run lolcat -- -V
wapm execute lolcat -- -V
wapm uninstall lolcat
wapm execute lolcat -- -V
wapm list -a

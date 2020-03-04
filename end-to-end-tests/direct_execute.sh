#!/bin/sh

export RUST_BACKTRACE=1
alias wapm=target/debug/wapm
wapm config set registry.url "https://registry.wapm.dev"

wapm execute ls
wapm execute echo "hello"
wapm execute cowsay --emscripten "hello"
wapm list -a

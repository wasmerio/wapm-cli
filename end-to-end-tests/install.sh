#!/bin/sh

alias wapm=target/debug/wapm
wapm config set registry.url "https://registry.wapm.dev"
wapm install cowsay@0.1.2
wapm install cowsay@0.1.2
wapm run cowsay "hello, world"
wapm list
wapm uninstall cowsay
wapm install cowsay@0.1.2
wapm uninstall cowsay
wapm uninstall cowsay
wapm install -g mark/rust-example@0.1.9
wapm run hq9+ -e "H"
wapm uninstall -g mark/rust-example
wapm list -a

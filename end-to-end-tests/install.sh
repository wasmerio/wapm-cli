#!/bin/sh

alias wapm=target/debug/wapm
wapm config set registry.url "https://registry.wapm.dev"
wapm install cowsay@0.1.2
wapm run cowsay "hello, world"
wapm list
wapm uninstall cowsay
wapm list

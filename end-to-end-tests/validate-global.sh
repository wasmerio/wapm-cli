#!/bin/sh

alias wapm=target/debug/wapm
wapm config set registry.url "https://registry.wapm.dev"
wapm install -g mark2/binary-name-matters -y
wapm run binary-name-matters
wapm uninstall -g mark2/binary-name-matters
wapm install mark2/binary-name-matters -y
wapm run binary-name-matters
wapm uninstall mark2/binary-name-matters

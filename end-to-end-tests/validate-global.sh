#!/bin/sh

alias wapm=target/debug/wapm
wapm config set registry.url "https://registry.wapm.dev"

# test that the command name is overriden by default
wapm install -g mark2/binary-name-matters@0.0.3 -y
wapm run binary-name-matters
wapm uninstall -g mark2/binary-name-matters
wapm install mark2/binary-name-matters@0.0.3 -y
wapm run binary-name-matters
wapm uninstall mark2/binary-name-matters

# disable command rename and manually reenable it with `wasmer-extra-flags`
wapm install -g mark2/binary-name-matters-2 -y
wapm run binary-name-matters-2
wapm uninstall -g mark2/binary-name-matters-2
wapm install mark2/binary-name-matters-2 -y
wapm run binary-name-matters-2
wapm uninstall mark2/binary-name-matters-2

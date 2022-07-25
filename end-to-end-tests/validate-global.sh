#!/bin/sh

$WAPM config set registry.url "https://registry.wapm.dev"

# test that the command name is overriden by default
$WAPM install -g mark2/binary-name-matters@0.0.3 -y
$WAPM run binary-name-matters
$WAPM uninstall -g mark2/binary-name-matters
$WAPM install mark2/binary-name-matters@0.0.3 -y
$WAPM run binary-name-matters
$WAPM uninstall mark2/binary-name-matters

# disable command rename and manually reenable it with `wasmer-extra-flags`
$WAPM install -g mark2/binary-name-matters-2 -y
$WAPM run binary-name-matters-2
$WAPM uninstall -g mark2/binary-name-matters-2
$WAPM install mark2/binary-name-matters-2 -y
$WAPM run binary-name-matters-2
$WAPM uninstall mark2/binary-name-matters-2

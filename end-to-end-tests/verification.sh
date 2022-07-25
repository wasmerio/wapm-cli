#!/bin/sh

export RUST_BACKTRACE=1
$WAPM config set registry.url "https://registry.wapm.dev"
# redirect stderr to /dev/null so we can capture important stderr
yes no 2> /dev/null | $WAPM install mark2/dog2@0.0.0
# wc because the date changes
$WAPM keys list -a
yes 2> /dev/null | $WAPM install mark2/dog@0.0.4
$WAPM keys list -a | wc -l | xargs
$WAPM uninstall mark2/dog
$WAPM install mark2/dog@0.0.4
$WAPM install mark2/dog2@0.0.0
rm $HOME/.wasmer/wapm.sqlite &> /dev/null 
$WAPM install syrusakbary/dog3@0.0.0 --force-yes
$WAPM uninstall syrusakbary/dog3
$WAPM install syrusakbary/dog3@0.0.0 --force-yes

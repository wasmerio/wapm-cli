#!/bin/sh

export RUST_BACKTRACE=1
# redirect stderr to /dev/null so we can capture important stderr
yes no 2> /dev/null | wapm install mark2/dog2@0.0.0
# wc because the date changes
wapm keys list -a
yes 2> /dev/null | wapm install mark2/dog@0.0.4
wapm keys list -a | wc -l | xargs
wapm uninstall mark2/dog
wapm install mark2/dog@0.0.4
wapm install mark2/dog2@0.0.0
rm $HOME/.wasmer/wapm.sqlite &> /dev/null 
wapm install syrusakbary/dog3@0.0.0 --force-yes
wapm uninstall syrusakbary/dog3
wapm install syrusakbary/dog3@0.0.0 --force-yes

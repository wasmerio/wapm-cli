#!/bin/sh

#alias wapm=target/debug/wapm
wapm config set registry.url "https://registry.wapm.dev"
yes no | wapm install mark2/dog2@0.0.0
wapm keys list -a
yes | wapm install mark2/dog@0.0.4
wapm keys list -a
wapm uninstall mark2/dog
wapm install mark2/dog@0.0.4
wapm install mark2/dog2@0.0.0

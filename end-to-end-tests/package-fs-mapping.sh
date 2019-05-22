#!/bin/sh

alias wapm=target/debug/wapm
wapm config set registry.url "https://registry.wapm.dev"
yes | wapm install -g mark2/dog2@0.0.6
wapm run dog
wapm install mark2/dog2@0.0.6
wapm run dog

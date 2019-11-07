#!/bin/sh

alias wapm=target/debug/wapm
mkdir test-package
cd test-package
wapm config set registry.url "https://registry.wapm.dev"
wapm init -y
wapm add this-package-does-not-exist
wapm add mark2/python@0.0.4 mark2/dog2
cat wapm.toml
cd ..
rm -rf test-package

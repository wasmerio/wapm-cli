#!/bin/sh

mkdir test-package
cd test-package
WAX=$(echo wapm execute)
wapm config set registry.url "https://registry.wapm.dev"
wapm init -y
wapm add this-package-does-not-exist
wapm add mark2/python@0.0.4 mark2/dog2
wapm add lolcat@0.1.1
wapm remove lolcat
cd ..
rm -rf test-package

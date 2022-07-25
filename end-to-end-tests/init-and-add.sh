#!/bin/sh

mkdir test-package
cd test-package
WAX=$(echo $WAPM execute)
$WAPM config set registry.url "https://registry.wapm.dev"
$WAPM init -y
$WAPM add this-package-does-not-exist
$WAPM add mark2/python@0.0.4 mark2/dog2
$WAPM add lolcat@0.1.1
$WAPM remove lolcat
cd ..
rm -rf test-package

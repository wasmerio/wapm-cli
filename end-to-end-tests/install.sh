#!/bin/sh

$WAPM config set registry.url "https://registry.wapm.dev"
$WAPM install namespace-example/cowsay@0.1.2
$WAPM install namespace-example/cowsay@0.1.2
$WAPM run cowsay "hello, world"
$WAPM list
$WAPM uninstall namespace-example/cowsay
$WAPM install namespace-example/cowsay@0.1.2
$WAPM uninstall namespace-example/cowsay
$WAPM uninstall namespace-example/cowsay
$WAPM install -g mark/rust-example@0.1.11
$WAPM run hq9+ -e "H"
$WAPM uninstall -g mark/rust-example
$WAPM install -g mark/wapm-override-test@0.1.0
$WAPM list -a
$WAPM run wapm-override-test
$WAPM install mark/wapm-override-test@0.2.0
$WAPM run wapm-override-test
$WAPM uninstall mark/wapm-override-test
$WAPM run wapm-override-test
$WAPM uninstall -g mark/wapm-override-test
$WAPM install namespace-example/cowsay@0.1.1 namespace-example/cowsay@0.1.2

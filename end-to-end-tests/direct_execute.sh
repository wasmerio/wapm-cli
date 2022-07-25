#!/bin/sh

export RUST_BACKTRACE=1
ln -sf `which wapm` wax
WAX=$(echo $WAPM execute)
$WAPM config set registry.url "https://registry.wapm.dev"

echo "hello" | wapm execute base64
$WAX echo "hello"
$WAPM install namespace-example/cowsay
$WAX --emscripten cowsay "hello"
$WAPM uninstall namespace-example/cowsay
$WAPM install lolcat
$WAPM run lolcat -V
$WAX  lolcat -V
$WAPM uninstall lolcat
$WAX lolcat -V
$WAPM list -a
rm -rf $(./wax --which lolcat)/wapm_packages/_/lolcat@0.1.1/*
$WAX lolcat -V
$WAX --offline lolcat -V
WAPM_RUNTIME=echo $WAX ls | grep "\-\-command-name" || echo "Success: command-name not found"

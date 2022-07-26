#!/bin/sh

export RUST_BACKTRACE=1
ln -sf `which wapm` wax
WAX=$(echo wapm execute)
wapm config set registry.url "https://registry.wapm.dev"

echo "hello" | wapm execute base64
$WAX echo "hello"
wapm install namespace-example/cowsay
$WAX --emscripten cowsay "hello"
wapm uninstall namespace-example/cowsay
wapm install lolcat
wapm run lolcat -V
$WAX  lolcat -V
wapm uninstall lolcat
$WAX lolcat -V
wapm list -a
rm -rf $(./wax --which lolcat)/wapm_packages/_/lolcat@0.1.1/*
$WAX lolcat -V
$WAX --offline lolcat -V
WAPM_RUNTIME=echo $WAX ls | grep "\-\-command-name" || echo "Success: command-name not found"

#!/usr/bin/env bash

export RUST_BACKTRACE=1
ln -sf `which wapm` wax
wapm config set registry.url "https://registry.wapm.dev/graphql"

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
PWD=$(pwd -P)
RANDOMVERSION1=$RANDOM
RANDOMVERSION2=$RANDOM
RANDOMVERSION3=$RANDOM

mkdir -p /tmp/largewasmfile
cp -rf $SCRIPT_DIR/../assets/largewasmfile.wasm /tmp/largewasmfile/largewasmfile.wasm
cp -rf $SCRIPT_DIR/../assets/largewasmfile.wapm.toml /tmp/largewasmfile/wapm.toml
cp -rf $SCRIPT_DIR/chunked_upload.txt /tmp/chunked_upload_reference.txt

if [ "$(uname)" == "Darwin" ]; then
    sed -i '' "s/RANDOMVERSION3/$RANDOMVERSION3/g" /tmp/largewasmfile/wapm.toml
    sed -i '' "s/RANDOMVERSION2/$RANDOMVERSION2/g" /tmp/largewasmfile/wapm.toml
    sed -i '' "s/RANDOMVERSION1/$RANDOMVERSION1/g" /tmp/largewasmfile/wapm.toml

    sed -i '' "s/RANDOMVERSION3/$RANDOMVERSION3/g" /tmp/chunked_upload_reference.txt
    sed -i '' "s/RANDOMVERSION2/$RANDOMVERSION2/g" /tmp/chunked_upload_reference.txt
    sed -i '' "s/RANDOMVERSION1/$RANDOMVERSION1/g" /tmp/chunked_upload_reference.txt
else
    sed -i "s/RANDOMVERSION3/$RANDOMVERSION3/g" /tmp/largewasmfile/wapm.toml
    sed -i "s/RANDOMVERSION2/$RANDOMVERSION2/g" /tmp/largewasmfile/wapm.toml
    sed -i "s/RANDOMVERSION1/$RANDOMVERSION1/g" /tmp/largewasmfile/wapm.toml

    sed -i "s/RANDOMVERSION3/$RANDOMVERSION3/g" /tmp/chunked_upload_reference.txt
    sed -i "s/RANDOMVERSION2/$RANDOMVERSION2/g" /tmp/chunked_upload_reference.txt
    sed -i "s/RANDOMVERSION1/$RANDOMVERSION1/g" /tmp/chunked_upload_reference.txt
fi

cd /tmp/largewasmfile
wapm login --token ${{ secrets.WAPM_DEV_TOKEN }}
FORCE_WAPM_USE_CHUNKED_UPLOAD=1 wapm publish
cd $PWD

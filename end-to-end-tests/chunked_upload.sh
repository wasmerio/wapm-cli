#!/usr/bin/env bash

export RUST_BACKTRACE=1
ln -sf `which wapm` wax

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
PWD=$(pwd -P)
RANDOMVERSION1=$RANDOM
RANDOMVERSION2=$RANDOM
RANDOMVERSION3=$RANDOM
WAPMUSERNAME=$WAPM_DEV_USERNAME

mkdir -p /tmp/largewasmfile
cp -rf $SCRIPT_DIR/../assets/largewasmfile.wasm /tmp/largewasmfile/largewasmfile.wasm
cp -rf $SCRIPT_DIR/../assets/largewasmfile.wapm.toml /tmp/largewasmfile/wapm.toml
cp -rf $SCRIPT_DIR/chunked_upload.txt /tmp/chunked_upload_reference.txt

if [ "$(uname)" == "Darwin" ]; then
    sed -i '' "s/RANDOMVERSION3/$RANDOMVERSION3/g" /tmp/largewasmfile/wapm.toml
    sed -i '' "s/RANDOMVERSION2/$RANDOMVERSION2/g" /tmp/largewasmfile/wapm.toml
    sed -i '' "s/RANDOMVERSION1/$RANDOMVERSION1/g" /tmp/largewasmfile/wapm.toml
    sed -i '' "s/WAPMUSERNAME/$WAPMUSERNAME/g" /tmp/largewasmfile/wapm.toml

    sed -i '' "s/RANDOMVERSION3/$RANDOMVERSION3/g" /tmp/chunked_upload_reference.txt
    sed -i '' "s/RANDOMVERSION2/$RANDOMVERSION2/g" /tmp/chunked_upload_reference.txt
    sed -i '' "s/RANDOMVERSION1/$RANDOMVERSION1/g" /tmp/chunked_upload_reference.txt
    sed -i '' "s/WAPMUSERNAME/$WAPMUSERNAME/g" /tmp/chunked_upload_reference.txt
else
    sed -i "s/RANDOMVERSION3/$RANDOMVERSION3/g" /tmp/largewasmfile/wapm.toml
    sed -i "s/RANDOMVERSION2/$RANDOMVERSION2/g" /tmp/largewasmfile/wapm.toml
    sed -i "s/RANDOMVERSION1/$RANDOMVERSION1/g" /tmp/largewasmfile/wapm.toml
    sed -i "s/WAPMUSERNAME/$WAPMUSERNAME/g" /tmp/largewasmfile/wapm.toml

    sed -i "s/RANDOMVERSION3/$RANDOMVERSION3/g" /tmp/chunked_upload_reference.txt
    sed -i "s/RANDOMVERSION2/$RANDOMVERSION2/g" /tmp/chunked_upload_reference.txt
    sed -i "s/RANDOMVERSION1/$RANDOMVERSION1/g" /tmp/chunked_upload_reference.txt
    sed -i "s/WAPMUSERNAME/$WAPMUSERNAME/g" /tmp/chunked_upload_reference.txt
fi

cd /tmp/largewasmfile
wapm publish --quiet
cd $PWD

export PATH=$PATH:$HOME/.cargo/bin
export PATH=$PATH:$HOME/.wasmer/bin
export WAPM_DISABLE_COLOR=true
rm -f $WASMER_DIR/.wax_index.toml
# TODO force clear cache
rm -f wapm.toml
rm -f wapm.lock
wapm config set registry.url "https://registry.wapm.dev/graphql"
chmod +x end-to-end-tests/chunked_upload.sh
echo "RUNNING SCRIPT..."
./end-to-end-tests/chunked_upload.sh &> /tmp/chunked_upload.txt
echo "GENERATED OUTPUT:"
cat /tmp/chunked_upload.txt
echo "COMPARING..."
diff -Bba /tmp/chunked_upload_reference.txt /tmp/chunked_upload.txt
export OUT=$?
if ( [ -d globals ] || [ -f wapm.log ] ) then
   { echo "globals or wapm.log found; these files should not be in the working directory"; exit 1; }
   else { true; }
fi

rm -f wapm.lock
rm -f wapm.toml
rm -rf wapm_packages
rm -f /tmp/chunked_upload.txt
if ( [ $OUT -ne 0 ] ) then { cat $HOME/.wasmer/wapm.log; } fi
exit $OUT
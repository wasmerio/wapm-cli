export PATH=$PATH:$HOME/.cargo/bin
export PATH=$PATH:$HOME/.wasmer/bin
export WAPM_DISABLE_COLOR=true
rm -f $WASMER_DIR/wapm.sqlite
rm -f $WASMER_DIR/globals/wapm.lock
rm -rf wapm_packages
rm -f wapm.toml
rm -f wapm.lock
chmod +x end-to-end-tests/verification.sh
wapm config set registry.url "https://registry.wapm.dev"
echo "RUNNING SCRIPT..."
./end-to-end-tests/verification.sh &> /tmp/verification-out.txt
echo "GENERATED OUTPUT:"
cat /tmp/verification-out.txt
echo "COMPARING..."
diff -Bba end-to-end-tests/verification.txt /tmp/verification-out.txt
export OUT=$?
if ( [ -d globals ] || [ -f wapm.log ] ) then { echo "globals or wapm.log found; these files should not be in the working directory"; exit 1; } else { true; } fi
rm -f wapm.lock
rm -f wapm.toml
rm -rf wapm_packages
rm -f /tmp/verification-out.txt
if ( [ $OUT -ne 0 ] ) then { cat $HOME/.wasmer/wapm.log; } fi
exit $OUT

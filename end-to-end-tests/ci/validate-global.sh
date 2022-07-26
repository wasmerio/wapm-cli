export PATH=$PATH:$HOME/.cargo/bin
export PATH=$PATH:$HOME/.wasmer/bin
export WAPM_DISABLE_COLOR=true
rm -f $HOME/.wasmer/wapm.sqlite
rm -f $HOME/.wasmer/globals/wapm.lock
rm -f wapm.lock
rm -f wapm.toml
rm -rf wapm_packages
chmod +x end-to-end-tests/validate-global.sh
wapm uninstall --global --all
echo "RUNNING SCRIPT..."
./end-to-end-tests/validate-global.sh &> /tmp/validate-global-out.txt
echo "GENERATED OUTPUT:"
cat /tmp/validate-global-out.txt
echo "EXPECTED OUTPUT:"
cat end-to-end-tests/validate-global.txt
echo "COMPARING..."
diff -Bba end-to-end-tests/validate-global.txt /tmp/validate-global-out.txt
export OUT=$?
if ( [ -d globals ] || [ -f wapm.log ] ) then { echo "globals or wapm.log found; these files should not be in the working directory"; exit 1; } else { true; } fi
rm -f wapm.lock
rm -f wapm.toml
rm -rf wapm_packages
rm -f /tmp/validate-global-out.txt
rm -f $HOME/.wasmer/wapm.sqlite
if ( [ $OUT -ne 0 ] ) then { cat $HOME/.wasmer/wapm.log; } fi
exit $OUT

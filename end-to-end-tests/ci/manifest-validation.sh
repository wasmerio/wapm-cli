export PATH=$PATH:$HOME/.cargo/bin
export PATH=$PATH:$HOME/.wasmer/bin
export WAPM_DISABLE_COLOR=true
rm -f $HOME/.wasmer/wapm.sqlite
rm -f $HOME/.wasmer/globals/wapm.lock
rm -f wapm.lock
rm -f wapm.toml
rm -rf wapm_packages
chmod +x end-to-end-tests/manifest-validation.sh
cargo build --release
echo "pwd"
pwd
WORKDIR=$(pwd)
WAPM_EXE=$(readlink -m $WORKDIR/target/release/wapm)
echo $WAPM_EXE
$WAPM_EXE uninstall --global --all
echo "RUNNING SCRIPT..."
WAPM=$WAPM_EXE ./end-to-end-tests/manifest-validation.sh &> /tmp/manifest-validation-out.txt
echo "GENERATED OUTPUT:"
cat /tmp/manifest-validation-out.txt
echo "EXPECTED OUTPUT:"
cat end-to-end-tests/manifest-validation.txt
echo "COMPARING..."
diff -Bba end-to-end-tests/manifest-validation.txt /tmp/manifest-validation-out.txt
export OUT=$?
if ( [ -d globals ] || [ -f wapm.log ] ) then { echo "globals or wapm.log found; these files should not be in the working directory"; exit 1; } else { true; } fi
rm -f wapm.lock
rm -f wapm.toml
rm -rf wapm_packages
rm -f /tmp/manifest-validation-out.txt
rm -f $HOME/.wasmer/wapm.sqlite
if ( [ $OUT -ne 0 ] ) then { cat $HOME/.wasmer/wapm.log; } fi
exit $OUT

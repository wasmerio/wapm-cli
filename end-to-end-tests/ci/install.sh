export PATH=$PATH:$HOME/.cargo/bin
export PATH=$PATH:$HOME/.wasmer/bin
export WAPM_DISABLE_COLOR=true
rm -f $WASMER_DIR/wapm.sqlite
rm -f $WASMER_DIR/globals/wapm.lock
rm -rf wapm_packages
rm -f wapm.toml
rm -f wapm.lock
chmod +x end-to-end-tests/install.sh
cargo build --release
echo "pwd"
pwd
WORKDIR=$(pwd)
WAPM_EXE=$(readlink -m $WORKDIR/target/release/wapm)
echo $WAPM_EXE
$WAPM_EXE uninstall --global --all
echo "RUNNING SCRIPT..."
WAPM=$WAPM_EXE ./end-to-end-tests/install.sh &> /tmp/install-out.txt
echo "GENERATED OUTPUT:"
cat /tmp/install-out.txt
echo "EXPECTED OUTPUT:"
cat end-to-end-tests/install.txt
echo "COMPARING..."
diff -Bba end-to-end-tests/install.txt /tmp/install-out.txt
export OUT=$?
if ( [ -d globals ] || [ -f wapm.log ] ) then { echo "globals or wapm.log found; these files should not be in the working directory"; exit 1; } else { true; } fi
rm -f wapm.lock
rm -f wapm.toml
rm -rf wapm_packages
rm -f /tmp/install-out.txt
if ( [ $OUT -ne 0 ] ) then { cat $HOME/.wasmer/wapm.log; } fi
exit $OUT

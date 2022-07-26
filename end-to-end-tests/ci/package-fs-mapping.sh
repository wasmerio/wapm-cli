export PATH=$PATH:$HOME/.cargo/bin
export PATH=$PATH:$HOME/.wasmer/bin
export WAPM_DISABLE_COLOR=true
rm -f $HOME/.wasmer/wapm.sqlite
rm -f $HOME/.wasmer/globals/wapm.lock
rm -rf wapm_packages
rm -f wapm.toml
rm -f wapm.lock
chmod +x end-to-end-tests/package-fs-mapping.sh
wapm uninstall --global --all
echo "RUNNING SCRIPT..."
./end-to-end-tests/package-fs-mapping.sh &> /tmp/package-fs-mapping-out.txt
echo "GENERATED OUTPUT:"
cat /tmp/package-fs-mapping-out.txt
echo "EXPECTED OUTPUT:"
cat end-to-end-tests/end-to-end-tests/package-fs-mapping.txt
echo "COMPARING..."
## hack to get the current directory in the expected output
#sed -i.bak "s/{{CURRENT_DIR}}/$(pwd | sed 's/\//\\\//g')/g" end-to-end-tests/package-fs-mapping.txt
diff -Bba end-to-end-tests/package-fs-mapping.txt /tmp/package-fs-mapping-out.txt
export OUT=$?
if ( [ -d globals ] || [ -f wapm.log ] ) then { echo "globals or wapm.log found; these files should not be in the working directory"; exit 1; } else { true; } fi
rm -f wapm.lock
rm -f wapm.toml
rm -rf wapm_packages
rm -f /tmp/package-fs-mapping-out.txt
rm -f $HOME/.wasmer/wapm.sqlite
if ( [ $OUT -ne 0 ] ) then { cat $HOME/.wasmer/wapm.log; } fi
exit $OUT

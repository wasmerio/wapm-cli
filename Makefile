release:
	cargo build --release
# TODO: add other features for proper release build

integration-tests:
	cargo test --features "integration_tests" integration_tests::

regression-tests:
	chmod +x end-to-end-tests/ci/direct-execution.sh
	./end-to-end-tests/ci/direct-execution.sh
	chmod -x end-to-end-tests/ci/direct-execution.sh
	echo "\n - name: 'Regression test: Install, Uninstall, Run, and List'"
	chmod +x end-to-end-tests/ci/install.sh
	./end-to-end-tests/ci/install.sh
	chmod -x end-to-end-tests/ci/install.sh
	echo "\nname: 'Regression test: verification and public key management'"
	chmod +x end-to-end-tests/ci/verification.sh
	./end-to-end-tests/ci/verification.sh
	chmod -x end-to-end-tests/ci/verification.sh
	echo "\n name: 'Regression test: manifest validation rejects invalid manifests'"
	chmod +x end-to-end-tests/ci/manifest-validation.sh
	./end-to-end-tests/ci/manifest-validation.sh
	chmod -x end-to-end-tests/ci/manifest-validation.sh
	echo "\n: 'Regression test: package fs and command rename'"
	chmod +x end-to-end-tests/ci/validate-global.sh
	./end-to-end-tests/ci/validate-global.sh
	chmod -x end-to-end-tests/ci/validate-global.sh
	echo "\n: 'Regression test: Init a Manifest and Add some dependencies'"
	chmod +x end-to-end-tests/ci/init-and-add.sh
	./end-to-end-tests/ci/init-and-add.sh
	chmod -x end-to-end-tests/ci/init-and-add.sh

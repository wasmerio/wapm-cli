release:
	cargo build --release
# TODO: add other features for proper release build

integration-tests:
	cargo test --features "integration_tests" integration_tests::

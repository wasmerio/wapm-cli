release:
	cargo build --release

integration-tests:
	cargo test --features "integration_tests" integration_tests::

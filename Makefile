IS_DARWIN := 0
IS_LINUX := 0
IS_WINDOWS := 0
IS_AMD64 := 0
IS_AARCH64 := 0

CARGO_BINARY ?= cargo
CARGO_TARGET ?=

# Test Windows apart because it doesn't support `uname -s`.
ifeq ($(OS), Windows_NT)
	# We can assume it will likely be in amd64.
	IS_AMD64 := 1
	IS_WINDOWS := 1
else
	# Platform
	uname := $(shell uname -s)

	ifeq ($(uname), Darwin)
		IS_DARWIN := 1
	else ifeq ($(uname), Linux)
		IS_LINUX := 1
	else
		# We use spaces instead of tabs to indent `$(error)`
		# otherwise it's considered as a command outside a
		# target and it will fail.
                $(error Unrecognized platform, expect `Darwin`, `Linux` or `Windows_NT`)
	endif

	# Architecture
	uname := $(shell uname -m)

	ifeq ($(uname), x86_64)
		IS_AMD64 := 1
	else ifneq (, $(filter $(uname), aarch64 arm64))
		IS_AARCH64 := 1
	else
		# We use spaces instead of tabs to indent `$(error)`
		# otherwise it's considered as a command outside a
		# target and it will fail.
                $(error Unrecognized architecture, expect `x86_64`, `aarch64` or `arm64`)
	endif
endif

ifeq ($(IS_DARWIN), 1)
	TARGET_DIR ?= target/*/release
else
	TARGET_DIR ?= target/release
endif

build-release:
ifeq ($(IS_DARWIN), 1)
	# We build it without bundling sqlite, as is included by default in macos
	$(CARGO_BINARY) build $(CARGO_TARGET) --release --no-default-features --features "full packagesigning telemetry update-notifications"
else
	$(CARGO_BINARY) build $(CARGO_TARGET) --release --features "telemetry update-notifications"
endif

release: build-release
	mkdir -p "package/bin"
ifeq ($(IS_WINDOWS), 1)
	cp $(TARGET_DIR)/wapm.exe package/bin/ &&\
	printf '@echo off\nwapm.exe execute %%*' > package/bin/wax.cmd &&\
	chmod +x package/bin/wax.cmd ;
else
ifneq (, $(filter 1, $(IS_DARWIN) $(IS_LINUX)))
	cp $(TARGET_DIR)/wapm package/bin/ &&\
	printf "#!/bin/bash\nwapm execute \"\$$@\"" > package/bin/wax &&\
	chmod +x package/bin/wax ;
else
	cp $(TARGET_DIR)/wapm package/bin/
ifeq ($(IS_DARWIN), 1)
	codesign -s - package/bin/wapm || true
endif
endif
endif
	cp LICENSE package/LICENSE
	tar -C package -zcvf wapm-cli.tar.gz LICENSE bin
	mkdir -p dist
	mv wapm-cli.tar.gz dist/

integration-tests:
	$(CARGO_BINARY) test --features "integration_tests" integration_tests::

regression-tests:
	chmod +x end-to-end-tests/ci/chunked-upload.sh
	./end-to-end-tests/ci/chunked-upload.sh
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

update-schema:
	curl --fail "$(shell wapm config get registry.url)/schema.graphql" > graphql/schema.graphql

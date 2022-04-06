IS_DARWIN := 0
IS_LINUX := 0
IS_WINDOWS := 0
IS_AMD64 := 0
IS_AARCH64 := 0

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

TARGET_DIR := target/release

build-release:
ifeq ($(IS_DARWIN), 1)
	# We build it without bundling sqlite, as is included by default in macos
	cargo build --release --no-default-features --features "full packagesigning telemetry update-notifications"
else
	cargo build --release --features "telemetry update-notifications"
endif

release: build-release
	mkdir -p "package/bin"
ifeq ($(IS_WINDOWS), 1)
	cp $(TARGET_DIR)/wapm.exe package/bin/ ;\
	printf '@echo off\nwapm.exe execute %%*' > package/bin/wax.cmd ;\
	chmod +x package/bin/wax.cmd ;
else
ifneq (, $(filter 1, $(IS_DARWIN) $(IS_LINUX)))
	cp $(TARGET_DIR)/wapm package/bin/ ;\
	printf "#!/bin/bash\nwapm execute \"\$$@\"" > package/bin/wax ;\
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
	cargo test --features "integration_tests" integration_tests::

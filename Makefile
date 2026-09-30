install:
	@case "`uname -s`/`uname -m`" in \
		Darwin/arm64|Darwin/aarch64) binary=bin/drop-darwin-arm64 ;; \
		Darwin/x86_64) binary=bin/drop-darwin-x86_64 ;; \
		*) echo "No prebuilt Drop binary for this platform" >&2; exit 1 ;; \
	esac; \
	mkdir -p "$(HOME)/.local/bin"; \
	install -m 755 "$$binary" "$(HOME)/.local/bin/drop"; \
	echo "Installed Drop to $(HOME)/.local/bin/drop"

CARGO_TARGET_DIR ?= target

build_binaries:
	mkdir -p bin
	cargo build --release --target x86_64-apple-darwin
	install -m 755 $(CARGO_TARGET_DIR)/x86_64-apple-darwin/release/drop bin/drop-darwin-x86_64
	cargo build --release --target aarch64-apple-darwin
	install -m 755 $(CARGO_TARGET_DIR)/aarch64-apple-darwin/release/drop bin/drop-darwin-arm64

# make build_macos version=0.2.2
build_macos: build_binaries
	make zip_macos_x86_64 version=$(version)
	make zip_macos_arm64 version=$(version)

# make zip_macos_x86_64 version=0.2.2
zip_macos_x86_64:
	cd $(CARGO_TARGET_DIR)/x86_64-apple-darwin/release && \
	tar -zcvf $(version)_Darwin_x86_64.tar.gz drop && \
	cd ../../../

# make zip_macos_arm64 version=0.2.2
zip_macos_arm64:
	cd $(CARGO_TARGET_DIR)/aarch64-apple-darwin/release && \
	tar -zcvf $(version)_Darwin_arm64.tar.gz drop && \
	cd ../../../

# make build_macos version=0.2.2
build_macos_release:
	make build_macos version=0.2.2

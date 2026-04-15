# CHV Makefile
# Quick commands for building, packaging, and local installation.

.PHONY: all build build-ui release dev-install clean test fmt

all: build build-ui

build:
	cargo build --workspace --release

build-ui:
	cd ui && npm install && npm run build

release:
	./scripts/build-release.sh

dev-install:
	sudo ./scripts/dev-install.sh

test:
	cargo test --workspace

fmt:
	cargo fmt --all

clean:
	cargo clean
	rm -rf dist/

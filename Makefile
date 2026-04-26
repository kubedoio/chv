# CHV Makefile
# Quick commands for building, packaging, and local installation.

.PHONY: all build build-ui release dev-install clean test fmt bump-version

BUMP_TYPE ?= build

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

bump-version:
	./scripts/bump-version.sh $(BUMP_TYPE)

clean:
	cargo clean
	rm -rf dist/

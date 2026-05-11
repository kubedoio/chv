# CHV Makefile
# Quick commands for building, testing, linting, packaging, and local installation.
#
# Common workflow:
#   make check          # Run all checks (fmt, lint, test, build, version)
#   make build          # Debug build
#   make build-release  # Release build + UI
#   make package-local  # Build .deb and .rpm packages
#   make package-smoke  # Verify packages
#   make clean          # Clean build artifacts

.PHONY: all build build-debug build-ui build-release release dev-install clean test fmt lint check
.PHONY: package-deb package-rpm package-local package-smoke package-check bump-version
.PHONY: integration-kvm integration-kvm-source integration-kvm-packages

BUMP_TYPE ?= patch

all: build build-ui

build:
	cargo build --workspace

build-debug: build

build-ui:
	cd ui && npm install && npm run build

build-release: build-ui
	cargo build --workspace --release

release: build-release
	./scripts/build-release.sh

dev-install:
	sudo ./scripts/dev-install.sh

test:
	cargo test --workspace

fmt:
	cargo fmt --all

lint:
	cargo clippy --workspace -- -D warnings

check:
	./scripts/check-release-local.sh

package-local: build-release
	./scripts/build-packages.sh

package-deb: build-release
	./scripts/build-packages.sh --format deb

package-rpm: build-release
	./scripts/build-packages.sh --format rpm

package-smoke:
	./scripts/smoke-packages.sh

package-check:
	./scripts/package/check-package-files.sh

package-safety:
	./scripts/package/check-safety.sh

package-smoke-deb:
	./scripts/package/smoke-deb.sh dist/packages

package-smoke-rpm:
	./scripts/package/smoke-rpm.sh dist/packages

integration-kvm: build-release package-deb
	sudo ./scripts/integration/kvm-smoke.sh --packages dist/packages

integration-kvm-source: build-release
	sudo ./scripts/integration/kvm-smoke.sh --source

integration-kvm-packages:
	sudo ./scripts/integration/kvm-smoke.sh --packages dist/packages

nightly: build-release package-deb package-rpm
	CHV_PKG_PRERELEASE="nightly.$$(date +%Y%m%d).g$$(git rev-parse --short HEAD)" \
		./scripts/build-packages.sh --skip-build

publish-repo-dry-run:
	@NIGHTLY_SUFFIX="nightly.$$(date +%Y%m%d).g$$(git rev-parse --short HEAD)"; \
	PKG_VERSION="$$(CHV_PKG_PRERELEASE="$$NIGHTLY_SUFFIX" ./scripts/version.sh --deb)"; \
	./scripts/publish/publish-repo.sh \
		--packages dist/packages \
		--channel nightly \
		--version "$$PKG_VERSION" \
		--dry-run

changelog:
	@VERSION="$$(cat VERSION)"; \
	./scripts/release/extract-changelog.sh "$$VERSION" || true

release-dry-run:
	@echo "Running release dry-run build..."
	@make build-release
	@make package-local
	@make package-check
	@make package-safety
	@make package-smoke-deb
	@make package-smoke-rpm
	@echo "Dry-run complete. Artifacts are in dist/"

sign-checksums:
	./scripts/release/sign-checksums.sh dist/packages/SHA256SUMS

verify-checksums:
	@cd dist/packages && sha256sum -c SHA256SUMS

package-lifecycle-deb:
	@if [ ! -d dist/packages-old ]; then \
		echo "Building old version packages for lifecycle test..."; \
		PACKAGE_VERSION=0.0.1 ./scripts/build-packages.sh --skip-build --format deb; \
		mkdir -p dist/packages-old; \
		cp dist/packages/*.deb dist/packages-old/; \
		echo "Building new version packages..."; \
		./scripts/build-packages.sh --skip-build --format deb; \
	fi
	./scripts/package/lifecycle-deb.sh --new-packages dist/packages --old-packages dist/packages-old

package-lifecycle-rpm:
	@if [ ! -d dist/packages-old ]; then \
		echo "Building old version packages for lifecycle test..."; \
		PACKAGE_VERSION=0.0.1 ./scripts/build-packages.sh --skip-build --format rpm; \
		mkdir -p dist/packages-old; \
		cp dist/packages/*.rpm dist/packages-old/; \
		echo "Building new version packages..."; \
		./scripts/build-packages.sh --skip-build --format rpm; \
	fi
	./scripts/package/lifecycle-rpm.sh --new-packages dist/packages --old-packages dist/packages-old

package-lifecycle: package-lifecycle-deb package-lifecycle-rpm

bump-version:
	./scripts/bump-version.sh $(BUMP_TYPE)

clean:
	cargo clean
	rm -rf dist/

# Convenience targets. All of these are thin wrappers around standard
# cargo commands — consumers without make can run the cargo commands directly.

.PHONY: build build-svg test coverage bump

## Build all crates.
build:
	cargo build --release

## Build and regenerate .github/assets/*.svg (always reruns build.rs).
build-svg:
	cargo build --release -p vibecheck-cli

## Run the full test suite.
test:
	cargo test --workspace

## Check coverage (vibecheck-core must hit 80%) and write lcov.info.
coverage:
	cargo llvm-cov --workspace --lcov --output-path lcov.info
	cargo llvm-cov --package vibecheck-core --fail-under-lines 80

## Bump workspace version: make bump LEVEL=patch|minor|major
bump:
	@test -n "$(LEVEL)" || { echo "Usage: make bump LEVEL=patch|minor|major"; exit 1; }
	./scripts/bump-version.sh $(LEVEL)

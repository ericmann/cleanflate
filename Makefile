.PHONY: fmt-check lint test corpus corpus-full crate-checks cleanroom-selftest \
        tools-check design-check ci proptest-10k bomb heavy \
        fuzz-check fuzz-smoke fuzz-long fuzz-divergences bench \
        provenance-snapshot provenance-summary build-report hooks-install \
        sources-check conformance-full malformed

fmt-check:
	cargo fmt --all -- --check

lint:
	cargo clippy --workspace --all-targets -- -D warnings

test:
	cargo test --workspace

corpus:
	@echo "STUB: corpus arrives in P1-03"

corpus-full:
	@echo "STUB: corpus-full arrives in P1-03"

conformance-full:
	@echo "STUB: conformance-full arrives in P1-03"

crate-checks:
	@echo "STUB: crate-checks arrives in P0-02"

cleanroom-selftest:
	@echo "STUB: cleanroom-selftest arrives in P0-05"

tools-check:
	@echo "STUB: tools-check arrives in P0-02"

design-check:
	@echo "STUB: design-check arrives in P1-05"

fuzz-check:
	@echo "STUB: fuzz-check arrives in P4-04"

fuzz-smoke:
	@echo "STUB: fuzz-smoke arrives in P4-04"

fuzz-long:
	@echo "STUB: fuzz-long arrives in P4-04"

fuzz-divergences:
	@echo "STUB: fuzz-divergences arrives in P4-04"

proptest-10k:
	@echo "STUB: proptest-10k arrives in P4-01"

bomb:
	@echo "STUB: bomb arrives in P4-02"

heavy:
	@echo "STUB: heavy arrives in P4-01"

malformed:
	@echo "STUB: malformed arrives in P2-04"

bench:
	@echo "STUB: bench arrives in P4-09"

sources-check:
	@echo "STUB: sources-check arrives in P0-07"

provenance-snapshot:
	@echo "STUB: provenance-snapshot arrives in P0-10"

provenance-summary:
	@echo "STUB: provenance-summary arrives in P6"

build-report:
	@echo "STUB: build-report arrives in P6"

hooks-install:
	@echo "STUB: hooks-install arrives in P0-06"

ci: fmt-check lint corpus test crate-checks cleanroom-selftest

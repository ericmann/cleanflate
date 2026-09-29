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
	cargo run --release -q -p oracle -- generate --tier quick

corpus-full:
	cargo run --release -q -p oracle -- generate --tier full

conformance-full:
	@echo "STUB: conformance-full arrives in P1-03"

crate-checks:
	bash tools/ci/crate-checks.sh

cleanroom-selftest:
	bash tools/cleanroom/selftest.sh

tools-check: sources-check
	@set -e; \
	if find tools -name '*.sh' -print -quit | grep -q .; then \
		find tools -name '*.sh' -print0 | xargs -0 shellcheck; \
	fi; \
	if [ -d tools/git-hooks ]; then \
		for f in tools/git-hooks/*; do \
			[ -f "$$f" ] && case "$$f" in *.sh) : ;; *) head -1 "$$f" | grep -qE '^#!.*(sh|bash)' && shellcheck "$$f" || true ;; esac; \
		done; \
	fi; \
	if find tools -name '*.py' -print -quit | grep -q .; then \
		find tools -name '*.py' -print0 | xargs -0 python3 -m py_compile; \
	fi; \
	if [ -d tools/cleanroom/tests ]; then \
		python3 -m unittest discover -s tools/cleanroom/tests -p 'test_*.py'; \
	fi; \
	if ls tools/ci/tests/*_test.sh >/dev/null 2>&1; then \
		for t in tools/ci/tests/*_test.sh; do \
			echo "== $$t =="; \
			bash "$$t"; \
		done; \
	fi; \
	if ls tools/git-hooks/tests/*_test.sh >/dev/null 2>&1; then \
		for t in tools/git-hooks/tests/*_test.sh; do \
			echo "== $$t =="; \
			bash "$$t"; \
		done; \
	fi

design-check:
	bash tools/ci/design-check.sh

fuzz-check:
	cargo check --manifest-path fuzz/Cargo.toml --bins

FUZZ_SECONDS ?= 60
FUZZ_TARGETS := never_panic_raw never_panic_rfc1950 never_panic_rfc1952 \
                 diff_oracle_roundtrip diff_oracle_arbitrary
FUZZ_SMOKE_TARGETS := $(addprefix fuzz-smoke-,$(FUZZ_TARGETS))

fuzz-smoke-%:
	@echo "== fuzz-smoke: $* =="
	@mkdir -p fuzz/corpus/$*
	cargo +nightly fuzz run $* fuzz/corpus/$* fuzz/seeds/$* -- -max_total_time=$(FUZZ_SECONDS) -rss_limit_mb=2048

fuzz-smoke: $(FUZZ_SMOKE_TARGETS)

# Target-specific variable (not a recursive $(MAKE) call) so `make -n
# fuzz-long` dry-runs all five 3600-second commands instead of make -n's
# usual "recursive $(MAKE) invocations always execute even under -n" trap.
fuzz-long: FUZZ_SECONDS := 3600
fuzz-long: $(FUZZ_SMOKE_TARGETS)

DIVERGENCE_LOG ?= fuzz/artifacts/divergences.jsonl

fuzz-divergences:
	@if [ -f $(DIVERGENCE_LOG) ]; then \
		jq -s 'group_by(.framing + "/" + .class) | map({key: (.[0].framing + "/" + .[0].class), count: length}) | sort_by(.key) | .[] | "\(.key): \(.count)"' -r $(DIVERGENCE_LOG); \
	else \
		echo "no divergence log at $(DIVERGENCE_LOG) (none recorded, or fuzz-smoke not yet run)"; \
	fi

proptest-10k:
	CLEANFLATE_CHUNKING_CASES=10000 cargo test --release -p oracle --test chunking

bomb:
	cargo test --release -p oracle --test bomb -- --include-ignored

heavy: proptest-10k bomb
	@echo "heavy: proptest-10k and bomb passed. fuzz-long (5 x 1h) is not run as part of heavy -- use 'make fuzz-long' or the fuzz-nightly workflow directly."

malformed:
	cargo run -q -p oracle -- malformed

bench:
	@echo "STUB: bench arrives in P4-09"

sources-check:
	bash tools/ci/sources-check.sh

provenance-snapshot:
	bash tools/cleanroom/snapshot-log.sh

provenance-summary:
	@echo "STUB: provenance-summary arrives in P6"

build-report:
	@echo "STUB: build-report arrives in P6"

hooks-install:
	git config core.hooksPath tools/git-hooks

ci: fmt-check lint corpus test crate-checks cleanroom-selftest tools-check design-check

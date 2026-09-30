.PHONY: regress-seed fmt-check lint test corpus corpus-full crate-checks cleanroom-selftest \
        tools-check design-check ci proptest-10k proptest-3k fastslow-full bomb heavy \
        fuzz-check fuzz-smoke fuzz-long fuzz-divergences bench bench-quick bench-compare bench-noise bench-md bench-stats profile-prep profile flamegraph \
        provenance-snapshot provenance-summary build-report hooks-install \
        sources-check conformance-full malformed

fmt-check:
	cargo fmt --all -- --check

lint:
	cargo clippy --workspace --all-targets -- -D warnings
	cargo clippy -p cleanflate --lib -- -D warnings
	cargo clippy -p cleanflate --all-targets --features stats -- -D warnings
	cargo clippy -p cleanflate --all-targets --features stats,test-hooks -- -D warnings
	cargo clippy -p oracle --all-targets --features stats -- -D warnings

test:
	cargo test --workspace
	cargo test -p cleanflate
	cargo test -p cleanflate --features stats

corpus:
	cargo run --release -q -p oracle -- generate --tier quick

corpus-full:
	cargo run --release -q -p oracle -- generate --tier full

conformance-full: corpus-full
	CLEANFLATE_TIER=full cargo test --release -p oracle --test conformance

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
                 diff_oracle_roundtrip diff_oracle_arbitrary diff_fast_reference
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
	CLEANFLATE_CHUNKING_CASES=10000 CLEANFLATE_FASTSLOW_CASES=10000 cargo test --release -p oracle --test chunking --test fastslow

bomb:
	cargo test --release -p oracle --test bomb -- --include-ignored

heavy: proptest-10k bomb conformance-full fastslow-full
	@echo "heavy: proptest-10k, bomb and conformance-full passed. fuzz-long (5 x 1h) is not run as part of heavy -- use 'make fuzz-long' or the fuzz-nightly workflow directly."

malformed:
	cargo run -q -p oracle -- malformed

BENCH_ID ?= adhoc
BENCH_OUT ?= target/bench/$(BENCH_ID).json

bench:
	mkdir -p $(dir $(BENCH_OUT))
	cargo run --release -q -p oracle -- bench --kind full --corpus silesia,canterbury \
		--framings raw,zlib,gzip --levels 1,6,9 --modes oneshot,streaming --iters 7 \
		--task $(BENCH_ID) --json $(BENCH_OUT)

bench-quick:
	mkdir -p $(dir $(BENCH_OUT))
	cargo run --release -q -p oracle -- bench --kind quick --subset quick --corpus silesia,canterbury \
		--framings raw,zlib,gzip --levels 6 --modes oneshot,streaming --iters 5 \
		--task $(BENCH_ID) --json $(BENCH_OUT)

sources-check:
	bash tools/ci/sources-check.sh

provenance-snapshot:
	bash tools/cleanroom/snapshot-log.sh

provenance-summary:
	python3 tools/cleanroom/summarize.py

build-report:
	python3 tools/cleanroom/build_report.py

hooks-install:
	git config core.hooksPath tools/git-hooks

ci: fmt-check lint corpus test crate-checks cleanroom-selftest tools-check design-check

proptest-3k:
	CLEANFLATE_CHUNKING_CASES=3000 CLEANFLATE_FASTSLOW_CASES=3000 cargo test --release -p oracle --test chunking --test fastslow

fastslow-full: corpus-full
	CLEANFLATE_TIER=full cargo test --release -p oracle --test fastslow

regress-seed:
	cargo run --release -q -p oracle -- regress-seed

bench-compare:
	@test -n "$(A)" -a -n "$(B)" || { echo "usage: make bench-compare A=before.json B=after.json [NOISE=noise.json]"; exit 2; }
	cargo run --release -q -p oracle -- bench-compare $(A) $(B) $(if $(NOISE),--noise $(NOISE))

bench-noise:
	@test -n "$(A)" -a -n "$(B)" -a -n "$(OUT)" || { echo "usage: make bench-noise A=run1.json B=run2.json OUT=docs/perf/noise.json"; exit 2; }
	cargo run --release -q -p oracle -- bench-noise $(A) $(B) --out $(OUT)

bench-md:
	@test -n "$(J)" || { echo "usage: make bench-md J=results.json"; exit 2; }
	cargo run --release -q -p oracle -- bench-md $(J)

bench-stats:
	mkdir -p $(dir $(BENCH_OUT))
	cargo run --release -q -p oracle --features stats -- bench --stats --iters 0 --kind custom --corpus silesia \
		--framings raw,zlib,gzip --levels 6 --modes oneshot,streaming --task $(BENCH_ID) --json $(BENCH_OUT)

PROFILE_CORPUS ?= silesia
PROFILE_FRAMING ?= rfc1951
PROFILE_LEVEL ?= 6
PROFILE_ITERS ?= 3
PROFILE_MODE ?= oneshot
PROFILE_PERF_FLAGS ?= --call-graph dwarf -F 499
PROFILE_DIR := testdata/perf-inputs/$(PROFILE_CORPUS)/$(PROFILE_FRAMING)-l$(PROFILE_LEVEL)
PROFILE_ARGS = --dir $(PROFILE_DIR) --format $(PROFILE_FRAMING) --iters $(PROFILE_ITERS) --mode $(PROFILE_MODE)

profile-prep:
	cargo run --release -q -p oracle -- perf-prep --corpus $(PROFILE_CORPUS) --framing $(PROFILE_FRAMING) --level $(PROFILE_LEVEL) --out testdata/perf-inputs

profile: profile-prep
	CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release -p cleanflate --example profile_decode
	mkdir -p target/profile
	perf record $(PROFILE_PERF_FLAGS) -o target/profile/perf.data target/release/examples/profile_decode $(PROFILE_ARGS)
	perf report -i target/profile/perf.data --stdio --no-children -g none --sort symbol --percent-limit 0.5 > target/profile/top.txt
	head -n 60 target/profile/top.txt

flamegraph: profile-prep
	mkdir -p target/profile
	CARGO_PROFILE_RELEASE_DEBUG=true cargo flamegraph -p cleanflate --example profile_decode -o target/profile/flamegraph.svg -- $(PROFILE_ARGS)
	@if [ -f perf.data ]; then mv -f perf.data target/profile/flamegraph-perf.data; fi

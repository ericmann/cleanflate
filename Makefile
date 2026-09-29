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
	@echo "STUB: fuzz-check arrives in P4-04"

fuzz-smoke:
	@echo "STUB: fuzz-smoke arrives in P4-04"

fuzz-long:
	@echo "STUB: fuzz-long arrives in P4-04"

fuzz-divergences:
	@echo "STUB: fuzz-divergences arrives in P4-04"

proptest-10k:
	CLEANFLATE_CHUNKING_CASES=10000 cargo test --release -p oracle --test chunking

bomb:
	@echo "STUB: bomb arrives in P4-03"

heavy: proptest-10k
	@echo "STUB: further heavy targets (bomb, fuzz-long) arrive in later phases"

malformed:
	cargo run -q -p oracle -- malformed

bench:
	@echo "STUB: bench arrives in P4-09"

sources-check:
	bash tools/ci/sources-check.sh

provenance-snapshot:
	@echo "STUB: provenance-snapshot arrives in P0-10"

provenance-summary:
	@echo "STUB: provenance-summary arrives in P6"

build-report:
	@echo "STUB: build-report arrives in P6"

hooks-install:
	git config core.hooksPath tools/git-hooks

ci: fmt-check lint corpus test crate-checks cleanroom-selftest tools-check design-check

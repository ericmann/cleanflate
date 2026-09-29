# human interventions

Manual log of every point a human operator intervened in the pipeline
(SPEC §5 M6). The automated provenance log (`tool-log.jsonl`) only
records tool calls the harness made itself; this table is filled in by
hand whenever a human did something the harness could not.

| when (UTC) | who | stage/task | what | why |
|------------|-----|------------|------|-----|
| 2026-09-29T15:33Z | operator | implement/P0-08 | Downloaded the Canterbury corpus archive (`cantrbry.tar.gz`, sha256 `f140e8a5b73d3f53198555a63bfb827889394a42f20825df33c810c3d5e3f8fb`) from the allowlisted URL in a browser and supplied it to `fetch-corpora.sh --from-file`, which verified it against that known-good SHA-256 before use. | `corpus.canterbury.ac.nz` returned HTTP 403 (Cloudflare bot protection) on every automated `curl` attempt from the build sandbox, including with browser-like headers (retried with a browser `User-Agent` header specifically, still blocked); Silesia downloaded automatically without issue. |
| 2026-09-29T18:50Z | operator (via controller session) | review→implement, round 1 | Re-opened the 17 tasks auto-skipped behind P0-08 (P0-09..P0-11, P1-03, P1-11, P1-12, P2-05, P2-06, P3-04, P3-06, P4-07..P4-10, P6-01..P6-03) by flipping `[-]` to `[ ]` in `docs/PROGRESS.md` (commit `b992fec`, tagged `[stage:review] [task:reopen]`). The operator authorized it; the go-flight controller session made the edit. | P0-08 was resolved with the operator-supplied archive, but Foundry has no tool to re-open skipped dependents, so they would never have run. |
| 2026-09-30T00:00Z | operator (via controller session) | post-approval CI | Two CI fixes after the round-4 APPROVED review, never reviewed by the pipeline: `ea3809a` commits `testdata/corpora/archives/cantrbry.tar.gz` (un-ignoring only that file) so CI skips the download; `f2b547b` makes the corpus verify step check only the requested corpus. | GitHub Actions runners also got HTTP 403 from the Canterbury host, and once past it, verify failed on Silesia files CI never fetches. |
| 2026-09-30 | operator | phase-2 pre-flight | Installed `perf` 7.1.5 (Pop!_OS `linux-tools-common`, `linux-tools-generic`, `linux-tools-$(uname -r)`) and `cargo flamegraph` 0.6.14 (`cargo install flamegraph --locked`). `kernel.perf_event_paranoid` left at 2. | Phase-2 SPEC §4.3/§8: profiling cleanflate's own code; agents cannot install tools after the network lockdown. |

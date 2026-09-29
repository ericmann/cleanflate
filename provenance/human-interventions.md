# human interventions

Manual log of every point a human operator intervened in the pipeline
(SPEC §5 M6). The automated provenance log (`tool-log.jsonl`) only
records tool calls the harness made itself; this table is filled in by
hand whenever a human did something the harness could not.

| when (UTC) | who | stage/task | what | why |
|------------|-----|------------|------|-----|
| 2026-09-29T15:33Z | operator | implement/P0-08 | Downloaded the Canterbury corpus archive (`cantrbry.tar.gz`, sha256 `f140e8a5b73d3f53198555a63bfb827889394a42f20825df33c810c3d5e3f8fb`) from the allowlisted URL in a browser and supplied it to `fetch-corpora.sh --from-file`, which verified it against that known-good SHA-256 before use. | `corpus.canterbury.ac.nz` returned HTTP 403 (Cloudflare bot protection) on every automated `curl` attempt from the build sandbox, including with browser-like headers (retried with a browser `User-Agent` header specifically, still blocked); Silesia downloaded automatically without issue. |

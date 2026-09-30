# Flamegraphs

Files are `<slug>-<before|after>.svg`, where the slug is `<corpus>-<rfc195x>-l<level>-<mode>`. They are written only by `make flamegraph-save PROFILE_KEY=... TAG=before|after` (or `make profile-required TAG=...`), from the cleanflate-only `profile_decode` example, and are checked by `tools/perf/check-flamegraph.sh` before they are copied here.

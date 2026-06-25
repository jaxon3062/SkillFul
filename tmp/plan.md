# Review fix stages

1. Add `tmp/` planning artifact, inspect current wrapper/event/config behavior, and confirm any existing proposal constraints.
2. Fix `wrap` privacy defaults so command lines are not stored by default, and preserve the child exit status on failure paths.
3. Fix `skill_end` success handling so omitted `--success` remains unknown/null instead of counting as failure; add regression tests.
4. Resolve skill definition file handling for `recommend` and default `unused` through configured/path-resolved lookup instead of raw relative `skills.toml`; add regression tests.
5. Run focused verification, then full `cargo test`, and commit each finished milestone with intentional messages.

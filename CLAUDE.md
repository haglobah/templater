# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

A conditional templating tool. Walks a source directory, copies each file to a destination, and rewrites it on the way through using `#if` / `#endif` directives gated on user-supplied flags. Used to scaffold new projects from `templates/` — e.g. `nix run github:haglobah/templater -- --to my_app devshell cljs haskell`.

**The destination is overwritten without mercy.** Code that changes output behavior should preserve this contract or call out the change loudly.

## Templating semantics

- `#if <cond>` on its own line opens a block; `#endif` closes it. Blocks nest and AND together with their parents.
- `#if <cond>` at the end of a content line is **inline**: the prefix is emitted only when the condition is true. Inline `#if` does **not** open a block — there is no matching `#endif`.
- Conditions: a bare flag `foo`, `(and a b ...)`, or `(or a b ...)`. No nested S-exprs, no negation.
- Labels: `#if <cond> <label>` and `#endif <label>` pair up. The implicit label of a single-flag `#if foo` is `foo`; complex conditions have no implicit label. `#endif` (bare) closes any open block; `#endif <label>` errors if it doesn't match the open's label. Stack frames in `process_content` carry `(active, Option<label>)`. Labels are also accepted on inline `#if`, where they are ignored.
- Trailing tokens after the condition/label are an error — no comment syntax. The trimmed line for `#endif` must be `#endif` or `#endif <single-token-label>`.
- Files whose processed output is entirely whitespace are skipped (no empty file written).
- Flags supplied on the CLI but never referenced in any condition are reported as "unused" with a Levenshtein suggestion. This is the user-facing typo guard — do not silently swallow unused flags.

## Common commands

```bash
cargo build                      # debug build
cargo test                       # all Rust tests (src/tests.rs)
cargo test test_process_and_true # single test by name
cargo run -- --to /tmp/out devshell rust   # run against ./templates

nix build .#rust                 # build Rust package
nix run . -- --to /tmp/out devshell rust   # uses bundled templates/
```

The dev shell (`nix develop`, auto-loaded via direnv) provides `rustc`, `cargo`, `rust-analyzer`, and `rustfmt`.

## Where things live

- `src/main.rs` — CLI, file walking, the `process_content` fold that drives the include-stack, and the unused-flag reporter.
- `src/tests.rs` — included via `mod tests` at the bottom of `main.rs`. Uses `Cursor` over string literals to drive `process_content` directly; that's the right level for new templating-behavior tests.
- `templates/` — the actual project skeleton shipped to users. Editing these is a user-facing change, not an implementation change.
- `flake.nix` — package + app + devshell + check definitions. `packages.default` is the shell wrapper that pins `--from` to the flake's bundled templates; downstream `nix run` users rely on this.

## Conventions worth knowing

- Errors use `anyhow` with `with_context` carrying file path and line number. Match that shape when adding new failure points — the test suite asserts on substrings like `"mismatched #endif"` and `"line 2"`.
- The Rust `process_content` is written as a `try_fold` over `(output, used_flags, stack)`. Keep it pure (no I/O) so tests can drive it from a `Cursor`.

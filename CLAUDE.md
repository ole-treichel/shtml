# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
cargo build           # Build all crates
cargo test            # Run all tests
cargo test <name>     # Run a single test by name
cargo test --features chaos  # Run tests including chaos feature tests
cargo test --features chaos,context  # Chaos + context (the primary mode)
```

CI runs `cargo build`, then `cargo test` for each feature combination (none, `chaos`, `context`, `chaos,context`) on push/PR to main.

## Architecture

**shtml** is a Rust library for server-side HTML rendering using a JSX-like macro syntax. It's a `no_std` crate (uses `alloc`).

### Two-crate workspace:

- **`shtml`** (`src/lib.rs`) — Public API: `Component` struct (wraps rendered HTML string), `Elements` type alias (children), `Render` trait (implemented for primitives, strings, `Vec<T>`), `escape()` function, and re-exports the `html!` macro as `view!` (also exported as `html!`). `src/context.rs` (behind `context` feature, pulls in `std`): thread-local, type-keyed context stack — `provide(value, f)`, `use_context::<T>()`, `expect_context::<T>()`.
- **`shtml_macros`** (`shtml_macros/src/lib.rs`) — Proc macro crate: `html!` macro parses JSX-like syntax via `rstml`, recursively renders nodes into an `Output` struct that combines static string segments with dynamic token streams. `chaos.rs` implements the `#[component]` attribute macro (behind `chaos` feature flag) which transforms functions into a struct plus a builder, enabling flexible attribute ordering and skippable optional props. In chaos mode `view!` emits a builder chain (`Fn::builder().attr(val)....build()`) rather than a positional call.

### Key patterns:

- **Components** are PascalCase functions returning `Component`. They receive typed attributes as parameters and optionally an `elements: Elements` parameter for children.
- **Render trait** is the core abstraction — anything rendered inside `view!` must implement it. String content is automatically HTML-escaped; `Component` content is not (already rendered).
- **Spread attributes**: `{..expr}` where expr evaluates to `Vec<(T, T)>` of key-value pairs.
- **Fragments**: `<>...</>` for grouping without a wrapper element.
- **Chaos is the primary mode.** Design new component features for `#[component]` first.
- **Context props** (chaos + `context`): a `#[component]` param marked `#[context]` falls back to `context::expect_context::<T>()` (or `use_context::<T>()` for `Option<T>`) at `build()` when not passed. Explicit attr overrides. Must be owned `Clone + 'static`. Generated code uses absolute `::shtml::context::` paths (`extern crate self as shtml` makes them resolve in unit tests). No provider component possible: children are rendered eagerly before the parent body runs.
- **Optional props** (chaos only): a `#[component]` parameter typed `Option<T>` can be omitted at the call site (defaults to `None`) or passed as `Some(value)`. Non-`Option` params stay required and panic at `build()` if omitted. `Option<&str>` is unsupported (needs a named lifetime); use `Option<String>`.

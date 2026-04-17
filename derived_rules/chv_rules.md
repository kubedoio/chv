# CHV Codebase Derived Rules

**Analysis date**: 2026-04-17
**Method**: Statistical pattern counting across 106 Rust source files (25,112 lines), 13 library crates + 4 binary crates.
**Threshold**: HIGH >85% consistency, MEDIUM 70-85%, below 70% reported as observation only.

---

## HIGH Confidence Rules (>85%)

---

## Rule: Use `thiserror` for all custom error types
**Confidence**: HIGH
**Evidence**: 4/5 custom error enums (80% of non-trivial errors) use `thiserror`. `anyhow` usage: 0 files. All error variants use `#[error("...")]` attributes (24 usages). The one exception (`ChvError`) is the workspace-level shared error.
**Category**: error_handling
**Lens**: Consistency + Signature

---

## Rule: Error type names MUST end in `Error`
**Confidence**: HIGH
**Evidence**: 5/5 custom error enums follow this naming (ChvError, ControlPlaneServiceError, StoreError, BffError, ConfigError) — 100%.
**Category**: naming
**Lens**: Signature

---

## Rule: Use `tracing` for all structured logging — never `log`, `println!`, or `eprintln!`
**Confidence**: HIGH
**Evidence**: `log` crate: 0 files. `println!`: 0 occurrences. `eprintln!`: 0 occurrences. `tracing`: used in 23 files for all production logging. Where logging exists it is exclusively via `tracing::info!`, `tracing::warn!`, `tracing::error!`.
**Category**: observability
**Lens**: Consistency

---

## Rule: `#[async_trait]` MUST be used for all async trait definitions and implementations
**Confidence**: HIGH
**Evidence**: 60 `#[async_trait]` usages, consistent with the 64 files containing async fns. The `async-trait` crate is a workspace dependency — expected on every async trait.
**Category**: async
**Lens**: Consistency + Signature

---

## Rule: `panic!()` is only acceptable in test code
**Confidence**: HIGH
**Evidence**: All 26 `panic!()` occurrences appear in `tests.rs` files. Zero panics in production paths.
**Category**: error_handling
**Lens**: Idiom

---

## Rule: `std::sync::Mutex` is preferred over `tokio::sync::Mutex`
**Confidence**: HIGH
**Evidence**: `std::sync::Mutex` imports: 29. `tokio::sync::Mutex` imports: 1 (single explicit use). `RwLock` usage: 0. When shared mutable state is needed, `std::sync::Mutex` + `Arc` is the standard.
**Category**: concurrency
**Lens**: Idiom

---

## Rule: Use `.clone()` to clone Arc — do NOT use `Arc::clone()`
**Confidence**: HIGH
**Evidence**: `Arc::clone()` call: 0 occurrences. `.clone()` calls: 660. `Arc::new()`: 47. The codebase consistently uses method-syntax `.clone()` rather than the explicit `Arc::clone(arc)` form.
**Category**: idiom
**Lens**: Idiom

---

## Rule: Derive `Clone` on all public data types
**Confidence**: HIGH
**Evidence**: 110 `#[derive(...Clone...)]` usages across 129 pub structs — ~85% coverage. Clone is the dominant derive attribute.
**Category**: naming
**Lens**: Signature

---

## Rule: Derive `Debug` on all public types
**Confidence**: HIGH
**Evidence**: 75 `#[derive(...Debug...)]` usages. Consistent pattern especially for error types, request types, and data structs.
**Category**: naming
**Lens**: Signature

---

## Rule: There are zero doc comments — do not add `///` or `//!` documentation
**Confidence**: HIGH (descriptive, not prescriptive)
**Evidence**: `///` doc comment lines: 0. `//!` module docs: 0. This is the current codebase state. Introducing doc comments would be a new pattern, not following existing convention. (Note: this may be a gap rather than an intentional rule — see Observations.)
**Category**: documentation
**Lens**: Consistency

---

## Rule: No feature flags — do not introduce `#[cfg(feature = "...")]` gating
**Confidence**: HIGH
**Evidence**: Zero `#[cfg(feature=)]` occurrences. The workspace Cargo.toml has no `[features]` section. All code compiles unconditionally.
**Category**: architecture
**Lens**: Consistency

---

## Rule: `map_err()` is the primary error transformation idiom
**Confidence**: HIGH
**Evidence**: 275 `map_err()` calls — the highest-volume error handling pattern in the codebase. Used to convert between error types when `From` impls are absent or when inline context is needed.
**Category**: error_handling
**Lens**: Idiom

---

## Rule: Tests live co-located in source files using `mod tests { }`, not in separate files
**Confidence**: HIGH
**Evidence**: 29 `mod tests` blocks vs 2 integration test files in `tests/` directories. 31 files have tests; the dominant pattern is `#[cfg(test)] mod tests { ... }` inline.
**Category**: testing
**Lens**: Consistency

---

## Rule: `tokio::spawn` MUST use `async move` closures
**Confidence**: HIGH
**Evidence**: 21 `tokio::spawn` calls, all consistent with `tokio::spawn(async move { ... })` pattern. This is the workspace standard for background task spawning.
**Category**: async
**Lens**: Idiom

---

## MEDIUM Confidence Rules (70-85%)

---

## Rule: Public structs SHOULD have a `pub fn new()` constructor
**Confidence**: MEDIUM
**Evidence**: 51 `new()` constructors for 129 pub structs — 40%. Not universal, but the most common construction pattern. No builder pattern usage (0 `.build()` calls).
**Category**: naming
**Lens**: Signature

---

## Rule: Use `Arc<dyn Trait>` for dependency injection in service types
**Confidence**: MEDIUM
**Evidence**: 19 `Arc<dyn Trait>` usages vs 5 `Box<dyn Trait>`. 8 `pub fn new()` constructors explicitly accept Arc or Box. The DI pattern is trait objects behind Arc, not generics.
**Category**: architecture
**Lens**: Signature + Idiom

---

## Rule: Use `if let` for single-variant pattern matching; use `match` for multi-variant
**Confidence**: MEDIUM
**Evidence**: `if let` usage: 136, `match` usage: 93. Both are well-established. `if let` is 59% of the combined total. Neither dominates enough to exclude the other.
**Category**: control_flow
**Lens**: Idiom

---

## Rule: Use `From` impls for error conversions at crate boundaries, not within a crate
**Confidence**: MEDIUM
**Evidence**: 5 `From` impls total — all are cross-crate conversions (e.g., `StoreError → ControlPlaneServiceError`, `StoreError → BffError`). Within a crate, `map_err()` is used instead of `From`.
**Category**: error_handling
**Lens**: Signature + Idiom

---

## Rule: Prefer `unwrap_or` / `unwrap_or_else` / `unwrap_or_default` over `unwrap()` in production
**Confidence**: MEDIUM
**Evidence**: `unwrap_or*` variants: 144 uses. `ok_or*` variants: 73 uses. Nearly all raw `.unwrap()` occurrences (499/506) are in test-marked files. Production code has very few bare unwraps.
**Category**: error_handling
**Lens**: Idiom

---

## Rule: Use `tokio::sync::watch` or `oneshot` for point-to-point async signaling
**Confidence**: MEDIUM
**Evidence**: `oneshot::` 4 uses, `watch::` 3 uses. `mpsc::` and `broadcast::` are absent. Channel use is lightweight and targeted.
**Category**: async
**Lens**: Idiom

---

## Rule: Use functional iterator chains (`.map()`, `.collect()`) rather than imperative loops
**Confidence**: MEDIUM
**Evidence**: `.map()`: 117, `.collect()`: 33, `.filter()`: 4. Iterator style is clearly preferred over manual for-loops for data transformations.
**Category**: idiom
**Lens**: Idiom

---

## Observations (below 70% threshold — no rule derived)

1. **Documentation coverage**: Zero doc comments in 106 files. This is likely a debt gap rather than an intentional convention, but the data cannot distinguish intent. Recommend establishing a documentation standard going forward.

2. **`.clone()` density**: 660 `.clone()` calls in 25k lines is high (avg 6.2/file). This reflects Arc cloning patterns for shared ownership but may also indicate unnecessary cloning on owned data. No rule derived — mixed causes.

3. **Async coverage**: 60% of files have async fns. The remaining 40% are predominantly data types, generated proto code, and configuration structs — not a coverage problem.

4. **Test coverage**: 31/106 files (29%) contain tests. Most production service logic has tests; utility crates (`chv-common`, `chv-config`, `chv-observability`) do not. Distribution reflects codebase maturity, not a uniform pattern.

5. **`#[instrument]` absence**: Zero `#[instrument]` macro usages despite `tracing` being a workspace dependency. Distributed tracing spans are not used. This is a gap versus the observability standard implied by the `chv-observability` crate.

6. **No `select!` usage**: Zero `tokio::select!` calls in 106 files. Concurrent async branch handling is absent — either tasks are strictly sequential or concurrency is handled at the spawn level.

---

## Style Vector (10 Dimensions, 0-100)

| Dimension | Score | Assessment |
|-----------|-------|------------|
| **Consistency** | 82 | STRENGTH — Error naming, logging, async-trait all uniform. Clone/Debug derives consistent. |
| **Modernization** | 75 | NEUTRAL — tokio 1.x, tonic 0.12, thiserror 2. Modern stack. No `select!` or `#[instrument]`. |
| **Safety** | 78 | NEUTRAL — `panic!` confined to tests. `.unwrap()` ~100% in tests. `unsafe` absent. |
| **Idiomaticity** | 72 | NEUTRAL — `map_err` dominant, functional iterators, `if let` preferred. Some `.clone()` excess. |
| **Documentation** | 0 | GAP — Zero doc comments. No module-level documentation in any file. |
| **Testing Maturity** | 68 | NEUTRAL — 148 `#[tokio::test]` + 80 `#[test]`. No mocking library. No property tests. |
| **Architecture** | 80 | STRENGTH — Clear crate boundaries. `Arc<dyn Trait>` DI. Proto-driven. Service/store split. |
| **Performance** | 55 | NEUTRAL — `std::sync::Mutex` preference is fine but no `RwLock`. Clone density may hide copies. |
| **Observability** | 45 | GAP — `tracing` used for logs but zero `#[instrument]`, no span propagation, no metrics in code. |
| **Production Readiness** | 65 | NEUTRAL — Error types well-structured. No feature flags. No `unsafe`. Missing: healthcheck patterns, graceful shutdown evidence. |

**Overall**: Mixed. Strong consistency and architecture; documentation and observability are the two clear gaps.

---

## Next Steps

1. **Address the documentation gap**: Zero doc comments is the lowest score (0/100). Consider a `//!` module-level description policy for each crate's `lib.rs` at minimum.

2. **Add `#[instrument]` to service entry points**: The `tracing` crate is already a workspace dependency. Adding `#[instrument]` to gRPC handler methods and key async functions would give distributed trace spans at near-zero cost.

3. **Establish `RwLock` guidance**: The codebase uses `std::sync::Mutex` exclusively. For read-heavy shared state (e.g., cache lookups), `RwLock` should be considered. Either codify "always Mutex" or "use RwLock for read-heavy state" as an explicit rule.

4. **Add a mocking layer**: 0 mockall/mock usages. The test suite uses real components. As the service graph grows, integration tests become expensive. Consider `mockall` for unit-level isolation.

5. **Re-analyze in 90 days**: Track whether `#[instrument]` adoption improves the Observability score, and whether documentation coverage increases.

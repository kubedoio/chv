# Repository Direction

- Rust is the active backend and control-plane language for CHV.
- Go code under `/legacy/go-controlplane` is non-authoritative and kept only for reference or migration context.
- The source of truth lives in the Rust workspace (`/cmd`, `/crates`, `/gen/rust`) and the tracked repository specs/contracts (`/proto`, `/docs/specs`, `/docs/chv-llm-handoff-pack`).

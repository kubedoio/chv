# Repository Direction

- Rust is the active backend and control-plane language for CHV.
- The previous Go control-plane prototype was removed; Rust is the active backend direction.
- The source of truth lives in the Rust workspace (`/cmd`, `/crates`, `/gen/rust`) and the tracked repository specs/contracts (`/proto`, `/docs/specs`, `/docs/chv-llm-handoff-pack`).

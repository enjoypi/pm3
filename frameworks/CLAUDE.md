# frameworks — entry points and assembly

`main.rs` + DI assembly + route binding + log initialization + lifecycle. No business logic, no format conversion.

**MUST NOT depend on `usecases`/`entities` (enforced by `arch_tests`): all inner-layer types come from the named re-exports in `adapters`.**

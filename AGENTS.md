# pain-runtime — guide for AI agents

Runtime support library for the Pain language. It is a member of the workspace in the
parent repository (`pain-lng/pain`); `pain-compiler` depends on it by path.
General rules for the project are in the parent repository's `AGENTS.md`.

## Layout

| File | Responsibility |
| --- | --- |
| `src/object.rs` | `Value` (int, float, bool, string, none, object, list, array) and `ClassInstance` — the values the interpreter computes with |
| `src/allocator.rs` | `BumpAllocator`, `MemoryPool`, `Arena` |
| `src/gc.rs` | Mark-and-sweep `GarbageCollector` |
| `src/lib.rs` | Re-exports |

## Commands

From the workspace root:

```bash
cargo test -p pain-runtime
cargo clippy -p pain-runtime --all-targets -- -D warnings
```

## Things that are easy to get wrong

- **`Value` is public API for the interpreter.** `pain-compiler` matches on it
  exhaustively (`interpreter.rs`, `main.rs`), so adding or changing a variant is a
  change in two repositories: update the compiler in the same piece of work and
  bump both submodule pointers in the parent repository together.
- `Value` and `ClassInstance` are plain owned data (`Clone`): an object is copied
  when it is read from a variable. Reference semantics for objects would be a
  design change, not a local fix.
- `allocator.rs` and `gc.rs` contain `unsafe` code with manual layout and pointer
  arithmetic. Keep the existing invariants (alignment, bounds checks before
  writes), add a test for every new unsafe path, and do not introduce `unsafe`
  elsewhere without a reason.
- The interpreter currently uses only `object.rs`; the allocators and the GC are
  not wired into it. Do not assume they are exercised by the compiler's tests.

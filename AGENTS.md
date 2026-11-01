# Repository Guidelines

## Project Structure & Module Organization

DJejaj is a native Rust macOS DJ application in active development. `README.md` is the entry
point; `docs/PROJEKT.md` defines the product and architecture, and
`docs/ETAPY.md` defines implementation stages and acceptance criteria.
`docs/PRACA_NA_ZYWO.md` specifies electronic-music workflows, library browsing,
live protections, and sequencer behavior.
The workspace uses `crates/{app,audio,dsp,library,sampler}/` for the console,
device streaming, effects, SQLite/decoding, and synthesis/sequencing respectively.
Generated sounds belong in `assets/`; small redistributable audio fixtures
belong in `tests/fixtures/`. `scripts/` contains launch and macOS packaging helpers.

## Build, Test, and Development Commands

Use the pinned Rust toolchain and Xcode Command Line Tools:

- `cargo build --workspace --release`: build optimized application components.
- `cargo test --workspace`: run automated tests.
- `cargo fmt --all --check`: check Rust formatting.
- `cargo clippy --workspace --all-targets -- -D warnings`: reject lint warnings.

`sh scripts/run.sh` rebuilds release before opening the console.
`python3 scripts/package_macos.py` produces `output/DJ EJAJ.app`.
Use `DJEJAJ_DATA_DIR` for isolated test libraries and sessions.

## Coding Style & Naming Conventions

Use rustfmt defaults, four-space indentation, `snake_case` for functions/modules,
`PascalCase` for types, and `SCREAMING_SNAKE_CASE` for constants. Keep DSP
independent of the GUI. Audio callbacks must avoid allocation, deallocation,
blocking synchronization, file/database access, and logging. Use bounded queues
and preallocated buffers. Write product documentation and interface copy in Polish.

## Testing Guidelines

Use Rust unit tests for DSP and integration tests for decoding, library storage,
transport, and keyboard contexts. Use descriptive names such as
`panic_mutes_effect_tails`. No coverage percentage is configured.
Include stereo isolation, parameter extremes, corrupted files, and focus changes.
Release performance and listening checks on actual macOS hardware must satisfy
`docs/ETAPY.md`; automated tests alone do not prove audio quality or latency.

## Commit & Pull Request Guidelines

This directory currently has no Git history, so no existing commit convention
can be inferred. When Git is initialized, use focused imperative commits, for
example `docs: define stereo acceptance criteria`. PRs should identify the stage,
describe behavior, link an issue if available, and report performed checks.
Include screenshots for UI changes and hardware/buffer details for audio measurements.

## Scope & Data Safety

Preserve original music files. Keep user libraries, recordings, secrets, and
unlicensed audio out of version control. Implement the documented stages only
when requested; distinguish proposed behavior from implemented and verified behavior.

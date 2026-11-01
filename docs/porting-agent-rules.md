# Rules for porting agents

Every agent working on the Rust/Bevy port reads this first, then `rust_port.md`
(status, findings, decisions D1-D16) and `c++_rust_bugs.md` (C++ bug ledger).
The plan is `docs/bevy-port-plan.md`.

## Workspace and ownership

- You work in your own git worktree of `/home/rucadi/m2rusty`. Other agents
  work in parallel in theirs. Edit **only the paths your brief says you own**.
  New crates go in `crates/<name>/`; the workspace picks them up by glob.
- Do **not** edit the root `Cargo.toml`, `Cargo.lock` (cargo will update it
  locally; leave that diff alone, the integrator regenerates it), `shell.nix`,
  `docs/`, `rust_port.md`, `c++_rust_bugs.md`, or C++ sources outside your
  `tools/differential/<module>/` directory. Reference other workspace crates by
  path in your own `Cargo.toml` (`m2-formats = { path = "../m2-formats" }`).
- Client data is `/home/rucadi/m2rusty/bin` (git-ignored, **read-only**). Read
  it through `m2_formats::pack::{PackManager, SearchMode, load_index}` with
  `load_index(&mut mgr, "pack", "pack/Index.dev")`. Tests that need it must skip
  with a message (not fail) when it is absent (`M2_CLIENT_DIR`, default `bin`).

## Commands

- Run EVERY command through `nix-shell --run '…'` from your worktree root. Never
  install anything globally.
- Crates that depend on Bevy: `export CARGO_TARGET_DIR=/home/rucadi/m2rusty/target`
  so Bevy is not rebuilt (about 11 minutes) in every worktree. Cargo serialises
  builds on that directory; waiting is expected. Never `cargo clean`.
- Crates that do not depend on Bevy: use the default target directory in your
  worktree (fast, no lock contention).
- Bevy is `=0.20.1`, newer than your training data. Do not guess its API: read
  `~/.cargo/registry/src/*/bevy*-0.20.1/` and compile early.

## Policies

- **D4 fidelity:** the Rust port must match the C++ on real data. Prove it with
  a differential test: a C++ dumper in `tools/differential/<module>/`
  (`CMakeLists.txt` defining `m2diff_<module>`, linking the REAL C++ code; the
  directory is auto-globbed; see `tools/differential/gr2/` and `race/` for the
  pattern) prints a canonical line-based dump; the Rust side prints the same;
  compare with `tests/support/differential.rs` helpers (copy that file's idea
  into your crate's `tests/support/` if your crate is not `m2-formats`). Floats
  print as IEEE bit hex. Run with `scripts/differential.sh` where it applies
  (builds with `-DM2_BUILD_DIFF_TOOLS=ON`, needs the nix-shell).
- **D5/D14 text:** public APIs take and return UTF-8 (`&str`/`String`) unless
  the data is genuinely binary. Legacy code pages are converted at the file
  boundary. Other encodings are not a goal.
- **D16 crates:** use a reputable crate instead of reimplementing, only if it is
  pure Rust (no C/C++ build step, no system library) and works on desktop,
  wasm32, Android and iOS. If a crate changes output compared with C++ on valid
  real data, the differential decides: port the C++ logic instead.
- **D7/D8 bugs:** malformed input returns an error, never a panic or hang. C++
  undefined behaviour is not mirrored. C++ bugs that change output on real data
  are mirrored with a comment `// C++ bug: …`. List every mirrored bug and
  every deviation in your final report (the integrator adds them to
  `c++_rust_bugs.md`).
- Portability: crates that are not about the C++ Python bridge must not depend
  on `m2-script` or any C/C++ code, so they stay wasm/mobile-capable.
- Style: doc comments say which C++ function each item ports; plain data types;
  small modules; unit tests for edge cases. The repo has no rustfmt config: if
  you format, use `max_width = 120, use_small_heuristics = "Max"` on your own
  files only.

## Finishing

- Done means: `cargo test -p <your crates>` passes, `cargo clippy -p <your
  crates> --all-targets` is clean, your differential tests pass, and your work is
  committed on your worktree branch. The commit message ends with
  `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`. Do not push or
  merge.
- If you hit a usage limit or must stop early, commit what works and say what is
  left.
- Final report, compact: branch + commit; public API; what the tests cover
  (counts on real data); every C++ bug mirrored or deviated from (ID-less is
  fine); anything deferred or uncertain; what other crates need from you or
  must provide to you.

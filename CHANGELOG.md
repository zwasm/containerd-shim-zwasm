# Changelog

All notable changes to this project will be documented in this file.

## [0.3.1] - 2026-11-23
### Security
- Moved to zwasm 2.12.1 through zwasm-sdk 0.8.1. On the JIT, which runs a guest
  by default, a valid module could take an `if`'s result from a value below it
  (GHSA-grpg-98rp-93g7, high), lose a value at a merge (GHSA-4wxj-2fwr-xxwq),
  end the shim's process while it was compiled (GHSA-pxpg-mx84-x9c9), catch a
  `throw_ref` of null that has to trap (GHSA-wwr2-84fx-2j37), or run on with a
  null reference after a GC allocation past the heap's cap, which now traps
  (GHSA-pjxf-4q9v-jpwq). On arm64, a function over about 1 MiB of machine code
  could end the process (GHSA-9v38-4fvh-wjp6). GHSA-493c-fvh2-2pv7 is Apple
  arm64 only and does not reach this shim

### Changed
- The JIT declines some modules it compiled before: one that uses `array.new`
  on an array of `v128` elements, and on arm64, one whose code is too far apart
  for a branch or a literal load, such as a function over about 1 MiB of
  machine code. The default engine runs such a module on the interpreter, unless
  the interpreter cannot run it either (it does not run `v128.const`, for one):
  then the container exits 1 without running the guest

### Fixed
- Three fixes without an advisory. On the JIT, an `if` with no else whose then
  arm branches to its own label gave the wrong values when the condition was
  zero. On both engines, a `try_table` catch clause whose label is the
  function's own did not return its values. On the arm64 JIT, `br_on_null` to a
  loop with no params was never taken, since this shim sets no fuel

## [0.3.0] - 2026-11-17
### Security
- Moved from zwasm 2.6.x to 2.12.0. A module's bytes alone could make the JIT,
  which runs a guest by default, read or write outside the GC heap
  (GHSA-4p74-j99r-p339, GHSA-rg2g-8x8j-3pm9, both high), or end the shim's
  process instead of trapping (GHSA-3983-xv7h-r8vx, GHSA-fv27-pjx7-p4m3, both
  medium). Two more end the process on the interpreter, which the default engine
  uses for a module the JIT declines (GHSA-xcf3-8ww5-fwxh, GHSA-ph9p-gw3q-7chp).
  The rest of zwasm's advisory list needs components, WASI 0.2 or 0.3, or
  `zwasm compile`, none of which this shim uses
- GHSA-vh35-jrc2-qj4q, a symlink escape from a preopen, did not reach outside
  the container here, since the shim's preopen is the container's own root. Its
  fix is the symlink change below

### Changed
- **A symlink whose target starts with `/` is no longer followed**, even when it
  resolves inside the preopen: reading through one fails with `notcapable`. A
  guest that reaches a file in its image through an absolute symlink stops
  working
- A module whose GC constant expression, element segment item or reference
  operand has the wrong type is rejected by validation, so one that loaded
  before now fails at module creation. This is the fix for GHSA-rg2g-8x8j-3pm9
  and GHSA-fv27-pjx7-p4m3
- Building needs Zig 0.17.1. zwasm pins one Zig minor exactly, so another fails
  inside its `build.zig`

## [0.2.0] - 2026-09-01
### Changed
- **Breaking for operators.** Moved to the zwasm 2.x C API through zwasm-sdk
  0.2. The SDK follows the wasm-c-api object model, so the shim now builds an
  `Engine` once and creates a `Store`, `Module` and `Instance` per container,
  mirroring runwasi's wasmtime shim
- The zwasm C library is linked statically. `libzwasm.so` no longer has to be
  deployed alongside the binary, and the install step is one file instead of two
- The entrypoint is resolved through a single path. `_start` is an ordinary
  export under the 2.x API, so the special case for it is gone
- `SIGTERM` no longer interrupts a running module. Every core module path in
  runwasi's own shims behaves this way; containerd stops an unresponsive
  container with `SIGKILL` after the termination grace period

### Fixed
- Exit codes are propagated. A guest that calls `exit(3)` gives a container that
  exited 3, and one that exits 0 is no longer reported as a failure. zwasm 2.6
  added a readable WASI exit status (zwasm/zwasm#234) and the SDK surfaces it as
  `Error::WasiExit`, so a clean exit and a trap are finally distinguishable
- Guests can reach the filesystem. Preopened directories now work: a guest can
  `path_open` and read a file under one. The zwasm v1 C API could not grant the
  capability
- Passing arguments no longer aborts the runtime. `args_sizes_get` and
  `args_get` return the container's arguments, `argv[0]` included, so a guest
  that skips `argv[0]` to read its arguments no longer loses the first one. The
  v1 line aborted on any non-empty argv, so no working behaviour changed here

## [0.1.1] - 2026-08-23
### Changed
- Dropped the section describing how to verify the shim on a kind cluster. It
  pointed at Dockerfiles and scripts kept in a separate repository, none of
  which this crate builds, tests or installs

No code changed, so the shim binary is identical to 0.1.0.

## [0.1.0] - 2026-08-13
### Added
- `ZwasmShim`, a `containerd-shim-wasm` shim that runs a container's
  WebAssembly module on zwasm. containerd starts it as the
  `io.containerd.zwasm.v1` runtime through the `containerd-shim-zwasm-v1` binary
- The container's arguments, environment and root directory are exposed to the
  module through WASI
- Cancellation on `SIGTERM`/`SIGINT`, reported as exit code 143
- Arguments and environment entries containing an interior NUL byte are dropped
  rather than truncating the list, since the zwasm C API takes NUL-terminated
  strings
- Unit tests for the sanitizing, and an integration suite that builds an OCI
  bundle and runs the shim against it
- Documentation covering building, installing and registering the shim with
  containerd, and using it from `ctr` and Kubernetes

### Known limitations
Three gaps that need a fix in zwasm rather than here, all still present in
v1.11.1, the last release of the zwasm v1 line this version builds against. The
integration tests covering the first two are `#[ignore]`.

- Exit codes are not propagated. `proc_exit` surfaces as a generic trap, so any
  module that exits non-zero is reported as exit code 1
- Guests cannot reach the filesystem. There is no C API to grant the WASI read,
  write and path capabilities, so every path operation fails with `EACCES` even
  for preopened directories
- Passing arguments aborts the runtime, because the C API reinterprets its array
  of argument pointers as an array of slices

Lifting these means following `zwasm-sdk` onto the redesigned v2 C API.

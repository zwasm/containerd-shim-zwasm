# Changelog

All notable changes to this project will be documented in this file.

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

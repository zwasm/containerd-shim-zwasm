# containerd-shim-zwasm

A [containerd](https://containerd.io) shim that runs WebAssembly containers on
the [zwasm](https://github.com/zwasm/zwasm) runtime.

The shim is built on [runwasi](https://github.com/containerd/runwasi)'s
`containerd-shim-wasm` and executes a container's WebAssembly module through
[zwasm-sdk](https://crates.io/crates/zwasm-sdk). It registers with containerd as
the `io.containerd.zwasm.v1` runtime.

## Requirements

- Linux (x86_64 or aarch64)
- containerd 2.x with the CRI plugin, if you want to use it from Kubernetes
- [Zig](https://ziglang.org/) 0.16.0 or later in `PATH` — the `zwasm-sys` build
  script compiles the zwasm C library from source
- A recent stable Rust toolchain (Rust 2021 edition)

## Build

```bash
cargo build --release
```

This produces two artifacts that have to be deployed together:

| Artifact | Location |
|----------|----------|
| `containerd-shim-zwasm-v1` | `target/release/` |
| `libzwasm.so` | `target/release/build/zwasm-sys-*/out/zig-install/lib/` |

The shim links against `libzwasm.so` dynamically and the binary carries no
`RUNPATH`, so the library must be reachable by the dynamic loader on the node.

## Install

Copy both artifacts onto the node and register the runtime with containerd:

```bash
install -m 0755 target/release/containerd-shim-zwasm-v1 /usr/local/bin/
install -m 0644 "$(find target/release -path '*/zig-install/lib/libzwasm.so' | head -n 1)" /usr/lib/
ldconfig
```

Add the runtime to `/etc/containerd/config.toml`:

```toml
[plugins."io.containerd.grpc.v1.cri".containerd.runtimes.zwasm]
  runtime_type = "io.containerd.zwasm.v1"
```

Then restart containerd:

```bash
systemctl restart containerd
```

## Usage

With `ctr`:

```bash
ctr run --rm --runtime io.containerd.zwasm.v1 \
  ghcr.io/containerd/runwasi/wasi-demo-app:latest demo
```

With Kubernetes, declare a `RuntimeClass` and reference it from a pod:

```yaml
apiVersion: node.k8s.io/v1
kind: RuntimeClass
metadata:
  name: zwasm
handler: zwasm
---
apiVersion: v1
kind: Pod
metadata:
  name: wasi-demo-app
spec:
  runtimeClassName: zwasm
  containers:
    - name: wasi-demo-app
      image: ghcr.io/containerd/runwasi/wasi-demo-app:latest
```

The container's arguments, environment variables and root directory are exposed
to the module through WASI. When containerd stops the container, the shim
cancels the running invocation on `SIGTERM`/`SIGINT` and reports exit code 143.

## Known limitations

The shim can only expose what the zwasm C API offers, and the current API leaves
three gaps. All of them need a fix in [zwasm](https://github.com/zwasm/zwasm)
itself — the runtime supports these features, they are simply not reachable
through `zwasm.h`.

- **Exit codes are not propagated.** `proc_exit` surfaces as a generic trap and
  the exit code recorded by the runtime (`getWasiExitCode`) has no C API. Any
  module that exits with a non-zero code is reported as exit code 1.
- **Guests cannot access the filesystem.** WASI capabilities default to stdio,
  clock, random and `proc_exit`; there is no C API to grant the read, write and
  path capabilities, so every path operation fails with `EACCES` even for
  preopened directories.
- **Passing arguments aborts the runtime.** A guest that reads its arguments
  crashes the process when argv is non-empty, because the C API reinterprets its
  array of argument pointers as an array of slices.

All three are still present in v1.11.1, the last release of the zwasm v1 line
that `zwasm-sdk` builds against; the line ended when the runtime was restarted
from scratch for v2. Lifting these limitations therefore means following
`zwasm-sdk` onto the redesigned v2 C API rather than waiting for a v1 fix.

The integration tests covering the first two are marked `#[ignore]`.

## Testing

Unit tests need no special privileges:

```bash
cargo test --lib instance::tests
```

The integration tests in `src/tests.rs` build an OCI bundle and run the shim
against it, which requires root:

```bash
sudo -E "$(command -v cargo)" test
```

Add `-- --ignored` to run the tests for the limitations listed above.

## Documentation

```bash
cargo doc --open
```

Documentation has to be built locally for now: docs.rs cannot build this crate
because the transitive `zwasm-sys` dependency invokes Zig from its build script,
which is not available in the docs.rs sandbox.

## Verifying on kind

The Dockerfiles and scripts used to build the shim for a
[kind](https://kind.sigs.k8s.io/) cluster and install it into the control plane
node are maintained in a separate repository,
[containerd-shim-zwasm-kind](https://github.com/jtakakura/containerd-shim-zwasm-kind).

## License

Licensed under the [MIT License](LICENSE).

# containerd-shim-zwasm

A [containerd](https://containerd.io) shim that runs WebAssembly containers on
the [zwasm](https://github.com/clojurewasm/zwasm) runtime.

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

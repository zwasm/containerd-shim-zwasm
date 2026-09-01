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
- [Zig](https://ziglang.org/) 0.16.0 or later in `PATH` to build — the
  `zwasm-sys` build script compiles the zwasm C library from source. Zig is not
  needed on the node that runs the shim
- A recent stable Rust toolchain (Rust 2021 edition)

## Build

```bash
cargo build --release
```

This produces a single artifact, `target/release/containerd-shim-zwasm-v1`. The
zwasm C library is linked statically, so nothing else has to be present on the
node.

## Install

Copy the binary onto the node and register the runtime with containerd:

```bash
install -m 0755 target/release/containerd-shim-zwasm-v1 /usr/local/bin/
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
to the module through WASI.

The module's exit code becomes the container's: a guest that calls `exit(3)`
gives a container that exited 3, and one that traps exits 1.

The shim does not interrupt a running module on `SIGTERM`, matching every core
module path in [runwasi](https://github.com/containerd/runwasi)'s own shims. A
container that does not return on its own is stopped by containerd's `SIGKILL`
once the termination grace period expires.

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

The crate is not published yet, so the documentation has to be built locally.

## License

Licensed under the [MIT License](LICENSE).

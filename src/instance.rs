//! The shim and sandbox implementations that run WebAssembly modules on zwasm.

use anyhow::{Context, Result};
use containerd_shim_wasm::sandbox::context::{Entrypoint, RuntimeContext};
use containerd_shim_wasm::sandbox::Sandbox;
use containerd_shim_wasm::shim::{version, Shim, Version};
use zwasm_sdk::{Engine, Error, Instance, Module, Store, Val, WasiConfig};

/// Drops arguments that cannot be passed to the zwasm C API.
///
/// The WASI configuration hands arguments to zwasm as NUL terminated strings,
/// so an argument containing an interior NUL byte has no representation there
/// and is skipped instead of truncating the whole argument list.
fn sanitize_args(args: &[&str]) -> Vec<String> {
    args.iter()
        .filter(|arg| !arg.contains('\0'))
        .map(|arg| (*arg).to_string())
        .collect()
}

/// Drops environment variables that cannot be passed to the zwasm C API.
///
/// See [`sanitize_args`] for why entries with interior NUL bytes are skipped.
fn sanitize_envs(envs: &[(&str, &str)]) -> Vec<(String, String)> {
    envs.iter()
        .filter(|(key, value)| !key.contains('\0') && !value.contains('\0'))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

/// The containerd shim for the zwasm WebAssembly runtime.
///
/// The shim is registered with containerd as `io.containerd.zwasm.v1` and is
/// started by the `containerd-shim-zwasm-v1` binary:
///
/// ```no_run
/// use containerd_shim_wasm::shim::Cli;
/// use containerd_shim_zwasm::ZwasmShim;
///
/// ZwasmShim::run(None);
/// ```
pub struct ZwasmShim;

/// The sandbox that executes a container's WebAssembly module on zwasm.
#[derive(Default)]
pub struct ZwasmSandbox {
    engine: Engine,
}

impl Shim for ZwasmShim {
    type Sandbox = ZwasmSandbox;

    fn name() -> &'static str {
        "zwasm"
    }

    fn version() -> Version {
        version!()
    }
}

impl Sandbox for ZwasmSandbox {
    /// Runs the module referenced by the container's entrypoint and returns its
    /// exit code.
    ///
    /// The container's arguments, environment and root directory are exposed to
    /// the module through WASI. The arguments are passed whole, so the guest's
    /// `argv[0]` is the entrypoint path, as it would be for a native process —
    /// dropping it would make a guest that skips `argv[0]`, which is the usual
    /// way to read arguments, lose its first real one. This is what runwasi's
    /// wasmtime shim does; its wasmer shim drops the entry instead.
    ///
    /// A guest that ends through WASI `proc_exit` reports the status it asked
    /// for, including zero — that is what a wasi-libc `_start` does on a clean
    /// run. A guest that traps is reported as exit code 1.
    ///
    /// A running module is not interrupted on `SIGTERM`; containerd stops one
    /// that does not return on its own with `SIGKILL`.
    async fn run_wasi(&self, ctx: &impl RuntimeContext) -> Result<i32> {
        let raw_args = ctx.args().iter().map(String::as_str).collect::<Vec<_>>();
        let raw_envs = ctx
            .envs()
            .iter()
            .map(|v| v.split_once('=').unwrap_or((v, "")))
            .collect::<Vec<_>>();
        let Entrypoint { source, func, .. } = ctx.entrypoint();

        let sanitized_args = sanitize_args(&raw_args);
        let sanitized_envs = sanitize_envs(&raw_envs);
        let args = sanitized_args
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let envs = sanitized_envs
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect::<Vec<_>>();

        let mut wasi_config = WasiConfig::new()?;
        wasi_config.set_args(&args)?;
        wasi_config.set_envs(&envs)?;
        wasi_config.preopen_dir("/", "/")?;

        let wasm_bytes = source.as_bytes()?;
        let mut store = Store::new(&self.engine)?;
        store.set_wasi(wasi_config);
        let module = Module::new(&mut store, &wasm_bytes)?;
        let instance = Instance::new(&mut store, &module, &[])?;
        let entry = instance
            .get_func(&mut store, &func)
            .with_context(|| format!("module does not export an entrypoint named {func:?}"))?;
        let mut results = vec![Val::I32(0); entry.result_arity(&store)];
        let result = entry.call(&mut store, &[], &mut results);

        match result {
            Ok(()) => {
                log::info!("wasm execution succeeded");
                Ok(0)
            }
            Err(Error::WasiExit { code }) => {
                log::info!("wasm exited with status {code}");
                Ok(code as i32)
            }
            Err(err) => {
                log::error!("failed to execute wasm: {err}");
                Ok(1)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{sanitize_args, sanitize_envs};

    #[test]
    fn sanitize_args_keeps_valid_arguments() {
        assert_eq!(
            sanitize_args(&["/hello.wasm", "--verbose", ""]),
            vec![
                "/hello.wasm".to_string(),
                "--verbose".to_string(),
                String::new()
            ]
        );
    }

    #[test]
    fn sanitize_args_drops_arguments_with_interior_nul() {
        assert_eq!(
            sanitize_args(&["ok", "bad\0arg", "also-ok"]),
            vec!["ok".to_string(), "also-ok".to_string()]
        );
    }

    #[test]
    fn sanitize_envs_keeps_valid_pairs() {
        assert_eq!(
            sanitize_envs(&[("PATH", "/bin"), ("EMPTY", "")]),
            vec![
                ("PATH".to_string(), "/bin".to_string()),
                ("EMPTY".to_string(), String::new()),
            ]
        );
    }

    #[test]
    fn sanitize_envs_drops_pairs_with_interior_nul() {
        assert_eq!(
            sanitize_envs(&[("BAD\0KEY", "value"), ("KEY", "bad\0value"), ("OK", "1")]),
            vec![("OK".to_string(), "1".to_string())]
        );
    }
}

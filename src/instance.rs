//! The shim and sandbox implementations that run WebAssembly modules on zwasm.

use anyhow::Result;
use containerd_shim_wasm::sandbox::context::{Entrypoint, RuntimeContext};
use containerd_shim_wasm::sandbox::Sandbox;
use containerd_shim_wasm::shim::{version, Shim, Version};
use tokio::signal::unix::{signal, SignalKind};
use zwasm_sdk::{Module, WasiConfig};

/// Exit code reported when execution is cancelled by a signal (128 + SIGTERM).
const EXIT_CODE_INTERRUPTED: i32 = 143;

/// The entrypoint function name a WASI command module exports.
const WASI_START_FUNCTION: &str = "_start";

/// Drops arguments that cannot be passed to the zwasm C API.
///
/// The WASI configuration hands arguments to zwasm as NUL terminated strings,
/// so an argument containing an interior NUL byte has no representation there
/// and is skipped instead of truncating the whole argument list.
fn sanitize_argv(args: &[&str]) -> Vec<String> {
    args.iter()
        .filter(|arg| !arg.contains('\0'))
        .map(|arg| (*arg).to_string())
        .collect()
}

/// Drops environment variables that cannot be passed to the zwasm C API.
///
/// See [`sanitize_argv`] for why entries with interior NUL bytes are skipped.
fn sanitize_env(envs: &[(&str, &str)]) -> Vec<(String, String)> {
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
pub struct ZwasmSandbox;

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
    /// the module through WASI. Execution is cancelled when the shim receives
    /// `SIGTERM` or `SIGINT`, in which case exit code 143 is returned.
    async fn run_wasi(&self, ctx: &impl RuntimeContext) -> Result<i32> {
        let args = ctx
            .args()
            .iter()
            .skip(1)
            .map(String::as_str)
            .collect::<Vec<_>>();
        let envs = ctx
            .envs()
            .iter()
            .map(|v| v.split_once('=').unwrap_or((v, "")))
            .collect::<Vec<_>>();
        let Entrypoint { source, func, .. } = ctx.entrypoint();

        let sanitized_args = sanitize_argv(&args);
        let sanitized_envs = sanitize_env(&envs);
        let argv = sanitized_args
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let env = sanitized_envs
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect::<Vec<_>>();

        let mut wasi_config = WasiConfig::new()?;
        wasi_config.set_argv(&argv)?;
        wasi_config.set_env(&env)?;
        wasi_config.preopen_dir("/", "/")?;

        let wasm_bytes = source.as_bytes()?;
        let module = Module::new_wasi_configured(&wasm_bytes, &wasi_config)?;

        // Cancel the running invocation when the shim is asked to stop. The
        // handle is thread safe, so it can outlive this task's scope.
        let cancel_handle = module.cancel_handle();
        let signal_task = tokio::spawn(async move {
            match (
                signal(SignalKind::terminate()),
                signal(SignalKind::interrupt()),
            ) {
                (Ok(mut sigterm), Ok(mut sigint)) => {
                    tokio::select! {
                        _ = sigterm.recv() => log::info!("received SIGTERM, cancelling execution"),
                        _ = sigint.recv() => log::info!("received SIGINT, cancelling execution"),
                    }
                    cancel_handle.cancel();
                }
                _ => log::debug!("signal handlers are not available in this environment"),
            }
        });

        log::info!("invoking {func}");
        let result = if func == WASI_START_FUNCTION {
            module.invoke_start()
        } else {
            module.invoke(&func, &[]).map(|_| ())
        };

        signal_task.abort();

        match result {
            Ok(()) => {
                log::info!("wasm execution succeeded");
                Ok(0)
            }
            Err(err) if err.is_interrupted() => {
                log::warn!("wasm execution was interrupted by a signal");
                Ok(EXIT_CODE_INTERRUPTED)
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
    use super::{sanitize_argv, sanitize_env};

    #[test]
    fn sanitize_argv_keeps_valid_arguments() {
        assert_eq!(
            sanitize_argv(&["/hello.wasm", "--verbose", ""]),
            vec![
                "/hello.wasm".to_string(),
                "--verbose".to_string(),
                String::new()
            ]
        );
    }

    #[test]
    fn sanitize_argv_drops_arguments_with_interior_nul() {
        assert_eq!(
            sanitize_argv(&["ok", "bad\0arg", "also-ok"]),
            vec!["ok".to_string(), "also-ok".to_string()]
        );
    }

    #[test]
    fn sanitize_env_keeps_valid_pairs() {
        assert_eq!(
            sanitize_env(&[("PATH", "/bin"), ("EMPTY", "")]),
            vec![
                ("PATH".to_string(), "/bin".to_string()),
                ("EMPTY".to_string(), String::new()),
            ]
        );
    }

    #[test]
    fn sanitize_env_drops_pairs_with_interior_nul() {
        assert_eq!(
            sanitize_env(&[("BAD\0KEY", "value"), ("KEY", "bad\0value"), ("OK", "1")]),
            vec![("OK".to_string(), "1".to_string())]
        );
    }
}

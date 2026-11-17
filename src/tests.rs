//! Integration tests that run WebAssembly modules end to end through the shim.
//!
//! [`WasiTest`] creates an OCI bundle and runs the shim against it, so these
//! tests need root privileges and are executed one at a time.

use std::os::unix::fs::symlink;
use std::path::Path;
use std::time::Duration;

use containerd_shim_wasm::testing::modules::*;
use containerd_shim_wasm::testing::WasiTest;
use serial_test::serial;

use crate::ZwasmShim as WasiEngine;

#[test]
#[serial]
fn test_delete_after_create() -> anyhow::Result<()> {
    WasiTest::<WasiEngine>::builder()?.build()?.delete()?;
    Ok(())
}

#[test]
#[serial]
fn test_hello_world() -> anyhow::Result<()> {
    let (exit_code, stdout, _) = WasiTest::<WasiEngine>::builder()?
        .with_wasm(HELLO_WORLD)?
        .build()?
        .start()?
        .wait(Duration::from_secs(10))?;

    assert_eq!(exit_code, 0);
    assert_eq!(stdout, "hello world\n");

    Ok(())
}

#[test]
#[serial]
fn test_hello_world_oci() -> anyhow::Result<()> {
    let (builder, _oci_cleanup) = WasiTest::<WasiEngine>::builder()?
        .with_wasm(HELLO_WORLD)?
        .as_oci_image(None, None)?;

    let (exit_code, stdout, _) = builder.build()?.start()?.wait(Duration::from_secs(10))?;

    assert_eq!(exit_code, 0);
    assert_eq!(stdout, "hello world\n");

    Ok(())
}

#[test]
#[serial]
fn test_custom_entrypoint() -> anyhow::Result<()> {
    let (exit_code, stdout, _) = WasiTest::<WasiEngine>::builder()?
        .with_start_fn("foo")
        .with_wasm(CUSTOM_ENTRYPOINT)?
        .build()?
        .start()?
        .wait(Duration::from_secs(10))?;

    assert_eq!(exit_code, 0);
    assert_eq!(stdout, "hello world\n");

    Ok(())
}

#[test]
#[serial]
fn test_unreachable() -> anyhow::Result<()> {
    let (exit_code, _, _) = WasiTest::<WasiEngine>::builder()?
        .with_wasm(UNREACHABLE)?
        .build()?
        .start()?
        .wait(Duration::from_secs(10))?;

    assert_ne!(exit_code, 0);

    Ok(())
}

#[test]
#[serial]
fn test_exit_code() -> anyhow::Result<()> {
    let (exit_code, _, _) = WasiTest::<WasiEngine>::builder()?
        .with_wasm(EXIT_CODE)?
        .build()?
        .start()?
        .wait(Duration::from_secs(10))?;

    assert_eq!(exit_code, 42);

    Ok(())
}

#[test]
#[serial]
fn test_seccomp() -> anyhow::Result<()> {
    let (exit_code, stdout, _) = WasiTest::<WasiEngine>::builder()?
        .with_wasm(SECCOMP)?
        .build()?
        .start()?
        .wait(Duration::from_secs(10))?;

    assert_eq!(exit_code, 0);
    assert_eq!(stdout.trim(), "current working dir: /");

    Ok(())
}

#[test]
#[serial]
fn test_has_default_devices() -> anyhow::Result<()> {
    let (exit_code, _, _) = WasiTest::<WasiEngine>::builder()?
        .with_wasm(HAS_DEFAULT_DEVICES)?
        .build()?
        .start()?
        .wait(Duration::from_secs(10))?;

    assert_eq!(exit_code, 0);

    Ok(())
}

/// Opens `probe` in the preopened root, following symlinks, and exits with the
/// errno `path_open` returned.
const OPEN_PROBE: &str = r#"
(module
  (import "wasi_snapshot_preview1" "path_open"
    (func $path_open (param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))
  (memory (export "memory") 1)
  (data (i32.const 16) "probe")
  (func (export "_start")
    (call $proc_exit
      (call $path_open
        (i32.const 3)   ;; fd: the preopen the shim makes for "/"
        (i32.const 1)   ;; lookupflags: symlink_follow
        (i32.const 16)  ;; path
        (i32.const 5)   ;; path length
        (i32.const 0)   ;; oflags
        (i64.const 2)   ;; rights: fd_read
        (i64.const 0)
        (i32.const 0)   ;; fdflags
        (i32.const 0))))) ;; where the opened fd is written
"#;

/// WASI preview 1 `errno::notcapable`.
const ERRNO_NOTCAPABLE: u32 = 76;

/// Runs [`OPEN_PROBE`] in a root holding `target.txt` and a `probe` symlink
/// pointing at `link_target`, and returns the guest's exit code.
fn open_through_symlink(link_target: impl AsRef<Path>) -> anyhow::Result<u32> {
    let test = WasiTest::<WasiEngine>::builder()?
        .with_wasm(wat::parse_str(OPEN_PROBE)?)?
        .build()?;

    let rootfs = test.root().join("rootfs");
    std::fs::write(rootfs.join("target.txt"), "")?;
    symlink(link_target, rootfs.join("probe"))?;

    let (exit_code, _, _) = test.start()?.wait(Duration::from_secs(10))?;
    Ok(exit_code)
}

#[test]
#[serial]
fn test_relative_symlink_is_followed() -> anyhow::Result<()> {
    assert_eq!(open_through_symlink("target.txt")?, 0);
    Ok(())
}

// zwasm 2.12.0 refuses any symlink whose target starts with `/`, even one that
// resolves inside the preopen. `test_relative_symlink_is_followed` reaches the
// same file, so the refusal comes from the absolute target and nothing else.
#[test]
#[serial]
fn test_absolute_symlink_is_refused() -> anyhow::Result<()> {
    assert_eq!(open_through_symlink("/target.txt")?, ERRNO_NOTCAPABLE);
    Ok(())
}

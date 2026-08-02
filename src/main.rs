//! The `containerd-shim-zwasm-v1` binary that containerd starts for the
//! `io.containerd.zwasm.v1` runtime.

use containerd_shim_wasm::shim::Cli;
use containerd_shim_zwasm::ZwasmShim;

fn main() {
    ZwasmShim::run(None);
}

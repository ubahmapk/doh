use std::sync::OnceLock;

/// The single tokio runtime backing both the blocking (`resolve`,
/// `.block_on`) and async (`aresolve`, via `pyo3-async-runtimes`) call
/// paths, so there is only ever one runtime driving network I/O regardless
/// of which style the caller uses.
pub fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Runtime::new().expect("failed to start the doh-core tokio runtime")
    })
}

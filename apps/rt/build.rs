fn main() {
    // Windows defaults the main-thread stack to 1 MiB, which is too small for
    // the debug build of the rt dispatcher (large monomorphized frames). Match
    // the POSIX default (8 MiB) so the binary does not stack-overflow on hook
    // dispatch. No-op on other targets.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rustc-link-arg-bins=/STACK:8388608");
    }
}

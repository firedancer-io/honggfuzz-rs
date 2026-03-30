use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(target_family = "windows")]
compile_error!("honggfuzz-rs does not currently support Windows but works well under WSL (Windows Subsystem for Linux)");

// TODO: maybe use `make-cmd` crate
#[cfg(not(any(
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd"
)))]
const GNU_MAKE: &str = "make";
#[cfg(any(
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "openbsd",
    target_os = "netbsd"
))]
const GNU_MAKE: &str = "gmake";

fn main() {
    println!("cargo:rustc-check-cfg=cfg(fuzzing)");
    println!("cargo:rustc-check-cfg=cfg(fuzzing_debug)");

    // Only build honggfuzz binaries if we are in the process of building an instrumentized binary
    let honggfuzz_target = match env::var("CARGO_HONGGFUZZ_TARGET_DIR") {
        Ok(path) => path, // path where to place honggfuzz binary. provided by cargo-hfuzz command.
        Err(_) => return,
    };

    // check that "cargo hfuzz" command is at the same version as this file
    let honggfuzz_build_version =
        env::var("CARGO_HONGGFUZZ_BUILD_VERSION").unwrap_or("unknown".to_string());
    if VERSION != honggfuzz_build_version {
        eprintln!("The version of the honggfuzz library dependency ({0}) and the version of the `cargo-hfuzz` executable ({1}) do not match.\n\
                   If updating both by running `cargo update` and `cargo install honggfuzz` does not work, you can either:\n\
                   - change the dependency in `Cargo.toml` to `honggfuzz = \"={1}\"`\n\
                   - or run `cargo install honggfuzz --version {0}`",
                  VERSION, honggfuzz_build_version);
        process::exit(1);
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap()); // from cargo
    let honggfuzz_target = Path::new(&env::var("CRATE_ROOT").unwrap()) // from honggfuzz
        .join(honggfuzz_target); // resolve the original honggfuzz_target relative to CRATE_ROOT

    // This crate lives at solfuzz/shlr/honggfuzz-rs, and the canonical
    // honggfuzz source is the sibling directory solfuzz/shlr/honggfuzz.
    // Use that instead of the embedded honggfuzz/ submodule to avoid
    // maintaining two copies.
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let honggfuzz_src = manifest_dir
        .join("../honggfuzz")
        .canonicalize()
        .expect("shlr/honggfuzz directory not found — is the solfuzz repo checked out correctly?");
    let hfuzz_src_str = honggfuzz_src.to_str().unwrap();

    // Check if the pre-built artifacts already exist (skip redundant rebuild)
    let libhfuzz_a = honggfuzz_src.join("libhfuzz/libhfuzz.a");
    let libhfcommon_a = honggfuzz_src.join("libhfcommon/libhfcommon.a");
    let honggfuzz_bin = honggfuzz_src.join("honggfuzz");

    if libhfuzz_a.exists() && libhfcommon_a.exists() && honggfuzz_bin.exists() {
        eprintln!(
            "honggfuzz-rs: reusing pre-built honggfuzz at {}",
            hfuzz_src_str
        );
    } else {
        // clean honggfuzz directory
        let status = Command::new(GNU_MAKE)
            .args(&["-C", hfuzz_src_str, "clean"])
            .status()
            .unwrap_or_else(|_e| {
                panic!("failed to run \"{} -C {} clean\"", GNU_MAKE, hfuzz_src_str)
            });
        assert!(status.success());

        // build honggfuzz command and hfuzz static library
        let status = Command::new(GNU_MAKE)
            .args(&["-C", hfuzz_src_str, "honggfuzz", "libhfuzz/libhfuzz.a", "libhfcommon/libhfcommon.a"])
            .status()
            .unwrap_or_else(|_e| panic!("failed to run \"{} -C {} honggfuzz libhfuzz/libhfuzz.a libhfcommon/libhfcommon.a\"", GNU_MAKE, hfuzz_src_str));
        assert!(status.success());
    }

    fs::copy(&libhfuzz_a, out_dir.join("libhfuzz.a")).unwrap();
    fs::copy(&libhfcommon_a, out_dir.join("libhfcommon.a")).unwrap();
    fs::copy(&honggfuzz_bin, honggfuzz_target.join("honggfuzz")).unwrap();

    // tell cargo how to link final executable to hfuzz static library
    println!("cargo:rustc-link-lib=static={}", "hfuzz");
    println!("cargo:rustc-link-lib=static={}", "hfcommon");
    println!("cargo:rustc-link-search=native={}", out_dir.display());
}

//! The Sentry DSN comes from the build environment (a CI secret, or a local
//! file the Makefile reads), never from the repository.

fn main() {
    println!("cargo:rerun-if-env-changed=NEWERA_SENTRY_DSN");
}

//! The Sentry DSN and the Google Analytics secret come from the build
//! environment (CI secrets, or a local file the Makefile reads), never from
//! the repository.

fn main() {
    println!("cargo:rerun-if-env-changed=NEWERA_SENTRY_DSN");
    println!("cargo:rerun-if-env-changed=NEWERA_GA_API_SECRET");
}

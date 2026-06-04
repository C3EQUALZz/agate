//! Integration tests: components against real infrastructure (PostgreSQL via
//! testcontainers; requires Docker — runs on the Linux CI leg). Wired as an
//! explicit `[[test]]` target so submodules nest under this directory.

mod dispatcher;
mod fixture;
mod gateway;

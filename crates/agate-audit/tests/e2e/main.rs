//! End-to-end tests: boot the full application and drive it over HTTP, then
//! verify the database state (requires Docker — runs on the Linux CI leg).
//! Wired as an explicit `[[test]]` target.

mod fixture;
mod http;

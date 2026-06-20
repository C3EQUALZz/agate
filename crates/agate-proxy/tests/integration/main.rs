//! Integration tests: components against real infrastructure — here the
//! upstream client against a booted stub AG-UI agent. Wired as an explicit
//! `[[test]]` target.

mod fixture;
#[cfg(feature = "redis")]
mod redis_session_memory;
mod upstream;

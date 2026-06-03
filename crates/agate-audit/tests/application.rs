//! Application-layer integration tests: use case handlers dispatched through
//! the mediator over in-memory fake gateways.
//!
//! One binary; the suites live in module folders for clarity. Real-database
//! adapter tests (testcontainers) belong to the infrastructure layer later.

mod commands;
mod common;
mod queries;

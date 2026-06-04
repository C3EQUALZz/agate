//! Application tests: use-case handlers dispatched through the mediator over
//! in-memory fakes — no real infrastructure (that is the `integration` and
//! `e2e` suites). Fast and isolated; submodules group them by command/query.

mod commands;
mod common;
mod queries;

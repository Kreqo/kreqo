// SPDX-FileCopyrightText: Kreqo contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tests that run xtask in throwaway git repositories, the way the hooks
//! and CI run it, so that each check keeps a test that fails without its
//! fix (QA-09). The tests that need the pinned Betterleaks or zizmor are
//! ignored by a plain `cargo test`; `cargo xtask check tool-tests` runs
//! them, as CI's checks job does.

mod hooks;
mod secrets;
mod support;
mod workflows;

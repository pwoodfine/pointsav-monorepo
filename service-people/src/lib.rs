// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

// Architectural Scaffold
pub mod acs;
pub mod fs_client;
pub mod http;
pub mod mcp;
pub mod people_store;
pub mod person;

pub use fs_client::FsClient;
pub use http::{router, AppState};
pub use people_store::PeopleStore;
pub use person::Person;

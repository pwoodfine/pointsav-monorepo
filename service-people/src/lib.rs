// SPDX-License-Identifier: Apache-2.0 OR MIT

pub mod person;

pub use person::Person;

pub fn system_status() -> &'static str {
    "SYSTEM EVENT: service-people scaffold verified."
}

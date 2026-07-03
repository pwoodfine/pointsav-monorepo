// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

#[derive(Debug, Clone, PartialEq)]
pub enum Tenant {
    Pointsav,
    Woodfine,
}

impl Tenant {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pointsav" => Some(Tenant::Pointsav),
            "woodfine" => Some(Tenant::Woodfine),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Tenant::Pointsav => "pointsav",
            Tenant::Woodfine => "woodfine",
        }
    }
}

#[derive(Debug, Clone)]
pub struct User {
    pub username: String,
    pub tenant: Tenant,
    pub role: String,
}

// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

use crate::vault::Layout;
use minijinja::{context, Environment};
use pulldown_cmark::{html, Options, Parser};
use std::collections::HashMap;

pub fn render_markdown(md: &str) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(md, opts);
    let mut out = String::new();
    html::push_html(&mut out, parser);
    out
}

pub fn render_nav(
    env: &Environment<'static>,
    nav: &HashMap<String, Vec<String>>,
    component_groups: &[(String, Vec<String>)],
    sections: &[(&str, &str, Layout)],
    active_section: &str,
    active_slug: &str,
) -> String {
    let section_names: Vec<&str> = sections.iter().map(|(s, _, _)| *s).collect();
    let default_tabs: HashMap<&str, &str> = sections.iter().map(|(s, t, _)| (*s, *t)).collect();
    env.get_template("nav.html")
        .expect("nav.html missing")
        .render(context! {
            nav => nav,
            component_groups => component_groups,
            sections => section_names,
            default_tabs => default_tabs,
            active_section => active_section,
            active_slug => active_slug,
        })
        .expect("render nav.html failed")
}

pub fn render_tab_bar(
    env: &Environment<'static>,
    section: &str,
    slug: &str,
    tabs: &[String],
    active_tab: &str,
) -> String {
    env.get_template("tab_bar.html")
        .expect("tab_bar.html missing")
        .render(context! {
            section => section,
            slug => slug,
            tabs => tabs,
            active_tab => active_tab,
        })
        .expect("render tab_bar.html failed")
}

pub fn shell(
    env: &Environment<'static>,
    title: &str,
    nav_html: &str,
    tab_bar: &str,
    page_title: &str,
    content: &str,
) -> String {
    let footer_html = env
        .get_template("footer.html")
        .expect("footer.html missing")
        .render(context! { version => env!("CARGO_PKG_VERSION") })
        .expect("render footer.html failed");
    env.get_template("shell.html")
        .expect("shell.html missing")
        .render(context! {
            title => title,
            nav_html => nav_html,
            tab_bar => tab_bar,
            page_title => page_title,
            content => content,
            footer_html => footer_html,
        })
        .expect("render shell.html failed")
}

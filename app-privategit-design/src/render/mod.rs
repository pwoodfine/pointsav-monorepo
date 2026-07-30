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
    sections: &[&str],
    active_section: &str,
    active_slug: &str,
) -> String {
    env.get_template("nav.html")
        .expect("nav.html missing")
        .render(context! {
            nav => nav,
            sections => sections,
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
    env.get_template("shell.html")
        .expect("shell.html missing")
        .render(context! {
            title => title,
            nav_html => nav_html,
            tab_bar => tab_bar,
            page_title => page_title,
            content => content,
        })
        .expect("render shell.html failed")
}

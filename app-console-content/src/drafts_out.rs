use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Write an accepted AI draft to `<outbound_dir>/<epoch>-<slug>.md` with
/// `foundry-draft-v1` frontmatter (Doctrine claim #39 — five research-trail fields).
pub fn write_draft(
    title: &str,
    protocol: &str,
    content: &str,
    tenant: &str,
    username: &str,
    outbound_dir: &str,
) -> Result<PathBuf> {
    let dir = Path::new(outbound_dir);
    fs::create_dir_all(dir)?;

    let epoch = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let slug = title_to_slug(title);
    let filename = format!("{}-{}.md", epoch, slug);
    let authored = epoch_to_iso(epoch);

    let front = format!(
        "---\n\
         schema: foundry-draft-v1\n\
         state: draft-ai-generated\n\
         originating_cluster: project-console\n\
         target_repo: \"\"\n\
         target_path: \"\"\n\
         target_filename: {filename}\n\
         language_protocol: {protocol}\n\
         authored: {authored}\n\
         authored_by: {username}@{tenant}\n\
         authored_with: claude-sonnet-4-6\n\
         research_done_count: 0\n\
         research_suggested_count: 0\n\
         open_questions_count: 0\n\
         research_provenance: []\n\
         research_inline: []\n\
         ---\n\n\
         # {title}\n\n",
        filename = filename,
        protocol = protocol,
        authored = authored,
        username = username,
        tenant = tenant,
        title = title,
    );

    let path = dir.join(&filename);
    fs::write(&path, format!("{}{}", front, content))?;
    Ok(path)
}

fn title_to_slug(title: &str) -> String {
    let raw: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    raw.split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
        .chars()
        .take(40)
        .collect()
}

fn epoch_to_iso(secs: u64) -> String {
    let sec = (secs % 60) as u32;
    let min = ((secs / 60) % 60) as u32;
    let hour = ((secs / 3600) % 24) as u32;
    let mut days = secs / 86400;
    let mut year = 1970u32;
    loop {
        let in_year = if is_leap(year) { 366u64 } else { 365u64 };
        if days < in_year {
            break;
        }
        days -= in_year;
        year += 1;
    }
    let month_days = [
        31u32,
        if is_leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u32;
    let mut remaining = days as u32;
    for &d in &month_days {
        if remaining < d {
            break;
        }
        remaining -= d;
        month += 1;
    }
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year,
        month,
        remaining + 1,
        hour,
        min,
        sec
    )
}

fn is_leap(y: u32) -> bool {
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

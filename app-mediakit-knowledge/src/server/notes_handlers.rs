// ── P6-F: Notes namespace (inline annotations) ───────────────────────────────
//
// GET  /notes/{slug}  — display article annotations with add-note form
// POST /notes/{slug}  — add a new annotation (SYS-ADR-10 F12 gate: confirm=yes)
//
// Storage: {content_dir}/annotations/{slug}.yaml (sidecar per article)
// Fields: id, anchor, author, body, created (ISO 8601), status (open|resolved)

use crate::annotations::{load_annotations, save_annotations, Annotation, AnnotationFile};

fn notes_page_chrome(
    slug: &str,
    site_title: &str,
    user: Option<&User>,
    pending_count: i64,
    annotation_list: &AnnotationFile,
) -> Markup {
    let article_url = format!("/wiki/{slug}");
    chrome(
        &format!("Notes: {slug} — {site_title}"),
        html! {
            div.wiki-title-row {
                nav.wiki-page-tabs aria-label="Page tabs" {
                    a.wiki-tab href=(article_url) { "Article" }
                    a.wiki-tab.wiki-tab-active aria-current="page" href={ "/notes/" (slug) } { "Notes" }
                }
                div.wiki-title-block {
                    h1.page-title { "Notes: " (slug) }
                    p.wiki-tagline { "Editorial annotations for this article" }
                }
            }

            // Annotation list
            @if annotation_list.annotations.is_empty() {
                p.wiki-notes-empty {
                    em { "No annotations yet. Use the form below to add the first note." }
                }
            } @else {
                div.notes-list {
                    @for note in &annotation_list.annotations {
                        div.note-card data-status=(note.status) id={ "note-" (note.id) } {
                            div.note-card__header {
                                span.note-card__anchor {
                                    @if !note.anchor.is_empty() {
                                        a href={ (article_url) "#" (note.anchor) } {
                                            "§ " (note.anchor)
                                        }
                                    } @else {
                                        "Article level"
                                    }
                                }
                                span.note-card__meta {
                                    (note.author) " · " (note.created)
                                    " · "
                                    @let status_cls = if note.status == "resolved" { "note-status note-status--resolved" } else { "note-status note-status--open" };
                                    span class=(status_cls) { (note.status) }
                                }
                            }
                            p.note-card__body { (note.body) }
                            @if !note.replies.is_empty() {
                                div.note-replies {
                                    @for reply in &note.replies {
                                        div.note-reply {
                                            span.note-reply__meta { (reply.author) " · " (reply.created) }
                                            p.note-reply__body { (reply.body) }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Add note form (always visible; SYS-ADR-10 F12 gate via confirm checkbox)
            section.wiki-notes-post {
                h2 { "Add a note" }
                form method="post" action={ "/notes/" (slug) } class="notes-form" {
                    div.notes-form__row {
                        label for="note-anchor" { "Heading anchor (optional)" }
                        input #note-anchor name="anchor" type="text"
                              placeholder="e.g. introduction (leave blank for article-level)";
                    }
                    div.notes-form__row {
                        label for="note-author" { "Your name" }
                        input #note-author name="author" type="text"
                              placeholder="Jennifer Woodfine" required;
                    }
                    div.notes-form__row {
                        label for="note-body" { "Note" }
                        textarea #note-body name="body" rows="4"
                                  placeholder="Your annotation…" required {}
                    }
                    // SYS-ADR-10 F12 gate — explicit operator confirmation required
                    div.notes-form__confirm {
                        label {
                            input type="checkbox" name="confirm" value="yes" required;
                            " I confirm this annotation is ready to record (F12)"
                        }
                    }
                    button.wiki-btn type="submit" { "Record annotation" }
                }
            }
        },
        site_title,
        user,
        pending_count,
    )
}

/// P6-F: `GET /notes/{*slug}` — serve notes page.
async fn notes_page(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    CurrentUser(maybe_user): CurrentUser,
) -> Result<Markup, WikiError> {
    if slug.contains("..") || slug.is_empty() {
        return Err(WikiError::NotFound(slug));
    }
    let pending_count = pending_count_for(&state, maybe_user.as_ref()).await;
    let annotations = load_annotations(state.primary_path(), &slug);
    Ok(notes_page_chrome(
        &slug,
        &state.site_title,
        maybe_user.as_ref(),
        pending_count,
        &annotations,
    ))
}

/// P6-F: `POST /notes/{*slug}` — record a new annotation (F12 gate).
async fn notes_post(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    CurrentUser(_maybe_user): CurrentUser,
    axum::extract::Form(form): axum::extract::Form<std::collections::HashMap<String, String>>,
) -> Response {
    if slug.contains("..") || slug.is_empty() {
        return (axum::http::StatusCode::BAD_REQUEST, "Invalid slug").into_response();
    }

    // SYS-ADR-10 F12 gate — reject if explicit confirmation not present
    if form.get("confirm").map(|s| s.as_str()) != Some("yes") {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            "F12 gate: confirmation required",
        )
            .into_response();
    }

    let anchor = form
        .get("anchor")
        .cloned()
        .unwrap_or_default()
        .trim()
        .to_string();
    let author = form
        .get("author")
        .cloned()
        .unwrap_or_default()
        .trim()
        .to_string();
    let body = form
        .get("body")
        .cloned()
        .unwrap_or_default()
        .trim()
        .to_string();

    if author.is_empty() || body.is_empty() {
        return (axum::http::StatusCode::BAD_REQUEST, "Author and body required")
            .into_response();
    }

    let mut file = load_annotations(state.primary_path(), &slug);

    // Short id: microseconds since epoch, base62-encoded (good enough for sidecar files)
    let id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| format!("{:x}", d.as_micros()))
        .unwrap_or_else(|_| "0".to_string());

    let created = {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        // ISO 8601 compact — precise enough for annotation records
        format!("{}Z", secs)
    };

    file.annotations.push(Annotation {
        id,
        anchor,
        author,
        body,
        created,
        status: "open".to_string(),
        replies: vec![],
    });

    if let Err(e) = save_annotations(state.primary_path(), &slug, &file) {
        tracing::error!("Failed to save annotation for {slug}: {e}");
        return (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to record annotation",
        )
            .into_response();
    }

    axum::response::Redirect::to(&format!("/notes/{slug}")).into_response()
}

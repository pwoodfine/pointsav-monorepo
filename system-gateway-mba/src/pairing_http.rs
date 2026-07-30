use anyhow::Result;
use serde_json::json;
use tiny_http::{Header, Request, Response, Server};

use crate::{
    db::{add_user, open_db},
    pairing::{
        new_code, normalize, ApproveBody, PairRequestBody, PairResponseBody, StatusResponseBody,
    },
    pairing_db::{
        get_by_code, get_state_by_id, insert_request, list_pending, set_state, sweep_expired,
    },
};

type BoxResp = Response<std::io::Cursor<Vec<u8>>>;

fn json_ok(body: serde_json::Value) -> BoxResp {
    Response::from_string(body.to_string())
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
}

fn json_err(status: u16, msg: &str) -> BoxResp {
    Response::from_string(json!({"error": msg}).to_string())
        .with_status_code(tiny_http::StatusCode(status))
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
}

fn read_body(req: &mut Request) -> Result<String> {
    let mut body = String::new();
    req.as_reader().read_to_string(&mut body)?;
    Ok(body)
}

fn handle_pair_request(req: &mut Request) -> BoxResp {
    let body_str = match read_body(req) {
        Ok(s) => s,
        Err(e) => return json_err(400, &format!("read error: {e}")),
    };
    let body: PairRequestBody = match serde_json::from_str(&body_str) {
        Ok(b) => b,
        Err(e) => return json_err(400, &format!("invalid json: {e}")),
    };

    if !["pointsav", "woodfine"].contains(&body.tenant.as_str()) {
        return json_err(400, "tenant must be 'pointsav' or 'woodfine'");
    }
    if body.username.is_empty() || body.fingerprint.is_empty() {
        return json_err(400, "username and fingerprint are required");
    }

    let conn = match open_db() {
        Ok(c) => c,
        Err(e) => return json_err(500, &format!("db error: {e}")),
    };
    let _ = sweep_expired(&conn);

    let code = new_code();
    let request_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now();
    let created_at = now.to_rfc3339();
    let expires_at = (now + chrono::Duration::seconds(600)).to_rfc3339();

    if let Err(e) = insert_request(&conn, &request_id, &code, &body, &created_at, &expires_at) {
        return json_err(500, &format!("insert error: {e}"));
    }

    let resp = PairResponseBody {
        request_id,
        code: code.clone(),
        expires_at: expires_at.clone(),
    };
    eprintln!(
        "pair-server: new request — {code} for {}@{}",
        body.username, body.tenant
    );
    json_ok(serde_json::to_value(resp).unwrap())
}

fn handle_status(url: &str) -> BoxResp {
    let request_id = url.trim_start_matches("/v1/pair/status/");
    if request_id.is_empty() {
        return json_err(400, "request_id required");
    }

    let conn = match open_db() {
        Ok(c) => c,
        Err(e) => return json_err(500, &format!("db error: {e}")),
    };
    let _ = sweep_expired(&conn);

    match get_state_by_id(&conn, request_id) {
        Ok(Some(state)) => {
            let resp = StatusResponseBody { state };
            json_ok(serde_json::to_value(resp).unwrap())
        }
        Ok(None) => json_err(404, "request not found"),
        Err(e) => json_err(500, &format!("db error: {e}")),
    }
}

fn handle_approve(req: &mut Request) -> BoxResp {
    let body_str = match read_body(req) {
        Ok(s) => s,
        Err(e) => return json_err(400, &format!("read error: {e}")),
    };
    let body: ApproveBody = match serde_json::from_str(&body_str) {
        Ok(b) => b,
        Err(e) => return json_err(400, &format!("invalid json: {e}")),
    };

    let normalized = normalize(&body.code);
    let conn = match open_db() {
        Ok(c) => c,
        Err(e) => return json_err(500, &format!("db error: {e}")),
    };
    let _ = sweep_expired(&conn);

    match get_by_code(&conn, &normalized) {
        Ok(Some((request_id, username, tenant, fingerprint, _public_key))) => {
            if let Err(e) = add_user(&conn, &fingerprint, &username, &tenant, "editor") {
                return json_err(500, &format!("add_user error: {e}"));
            }
            if let Err(e) = set_state(&conn, &request_id, "approved") {
                return json_err(500, &format!("state update error: {e}"));
            }
            eprintln!("pair-server: approved {username}@{tenant}  {fingerprint}");
            json_ok(json!({"status": "approved", "username": username, "tenant": tenant}))
        }
        Ok(None) => json_err(404, "code not found or already used"),
        Err(e) => json_err(500, &format!("db error: {e}")),
    }
}

fn handle_deny(req: &mut Request) -> BoxResp {
    let body_str = match read_body(req) {
        Ok(s) => s,
        Err(e) => return json_err(400, &format!("read error: {e}")),
    };
    let body: ApproveBody = match serde_json::from_str(&body_str) {
        Ok(b) => b,
        Err(e) => return json_err(400, &format!("invalid json: {e}")),
    };

    let normalized = normalize(&body.code);
    let conn = match open_db() {
        Ok(c) => c,
        Err(e) => return json_err(500, &format!("db error: {e}")),
    };

    match get_by_code(&conn, &normalized) {
        Ok(Some((request_id, username, _, _, _))) => {
            if let Err(e) = set_state(&conn, &request_id, "denied") {
                return json_err(500, &format!("state update error: {e}"));
            }
            eprintln!("pair-server: denied {username}");
            json_ok(json!({"status": "denied"}))
        }
        Ok(None) => json_err(404, "code not found or already used"),
        Err(e) => json_err(500, &format!("db error: {e}")),
    }
}

fn handle_pending() -> BoxResp {
    let conn = match open_db() {
        Ok(c) => c,
        Err(e) => return json_err(500, &format!("db error: {e}")),
    };
    let _ = sweep_expired(&conn);

    match list_pending(&conn) {
        Ok(rows) => {
            let items: Vec<_> = rows
                .iter()
                .map(|(id, code, user, tenant, created)| {
                    json!({
                        "request_id": id,
                        "code": code,
                        "username": user,
                        "tenant": tenant,
                        "created_at": created,
                    })
                })
                .collect();
            json_ok(json!({"pending": items}))
        }
        Err(e) => json_err(500, &format!("db error: {e}")),
    }
}

pub fn run_server(addr: &str) -> Result<()> {
    let server = Server::http(addr).map_err(|e| anyhow::anyhow!("{e}"))?;
    eprintln!("pair-server: listening on {addr}");

    for mut request in server.incoming_requests() {
        let method = request.method().to_string();
        let url = request.url().to_string();

        let resp: BoxResp = match (method.as_str(), url.as_str()) {
            ("POST", "/v1/pair/request") => handle_pair_request(&mut request),
            ("POST", "/v1/pair/approve") => handle_approve(&mut request),
            ("POST", "/v1/pair/deny") => handle_deny(&mut request),
            ("GET", "/v1/pair/pending") => handle_pending(),
            _ if method == "GET" && url.starts_with("/v1/pair/status/") => handle_status(&url),
            _ => json_err(404, "not found"),
        };

        let _ = request.respond(resp);
    }

    Ok(())
}

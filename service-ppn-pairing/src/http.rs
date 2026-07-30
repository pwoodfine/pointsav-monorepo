use anyhow::Result;
use serde_json::json;
use tiny_http::{Header, Request, Response, Server};

use system_pairing_codes::{
    new_code, normalize, ApproveBody, NodeJoinRequestBody, NodeJoinResponseBody, StatusResponseBody,
};

use crate::db::{
    get_by_code, get_state_by_id, insert_request, list_pending, open_db, register_approved_node,
    set_state, sweep_expired,
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

fn handle_join_request(req: &mut Request) -> BoxResp {
    let body_str = match read_body(req) {
        Ok(s) => s,
        Err(e) => return json_err(400, &format!("read error: {e}")),
    };
    let body: NodeJoinRequestBody = match serde_json::from_str(&body_str) {
        Ok(b) => b,
        Err(e) => return json_err(400, &format!("invalid json: {e}")),
    };

    if body.node_id.is_empty() || body.wireguard_pubkey.is_empty() {
        return json_err(400, "node_id and wireguard_pubkey are required");
    }
    if !["seL4", "netbsd-compat"].contains(&body.bottom.as_str()) {
        return json_err(400, "bottom must be 'seL4' or 'netbsd-compat'");
    }
    if !["aarch64", "x86_64"].contains(&body.arch.as_str()) {
        return json_err(400, "arch must be 'aarch64' or 'x86_64'");
    }

    let conn = match open_db() {
        Ok(c) => c,
        Err(e) => return json_err(500, &format!("db error: {e}")),
    };
    let _ = sweep_expired(&conn);

    let code = new_code();
    // Store the normalized (no-hyphen) form so approve/deny lookups match regardless
    // of whether the admin types "XXXX-XXXX" or "XXXXXXXX".
    let stored_code = normalize(&code);
    let request_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now();
    let created_at = now.to_rfc3339();
    let expires_at = (now + chrono::Duration::seconds(600)).to_rfc3339();

    if let Err(e) = insert_request(
        &conn,
        &request_id,
        &stored_code,
        &body,
        &created_at,
        &expires_at,
    ) {
        return json_err(500, &format!("insert error: {e}"));
    }

    eprintln!(
        "ppn-pairing: join request — {code} from {}  ({}/{})",
        body.node_id, body.bottom, body.arch
    );

    let resp = NodeJoinResponseBody {
        request_id,
        code,
        expires_at,
    };
    json_ok(serde_json::to_value(resp).unwrap())
}

fn handle_status(url: &str) -> BoxResp {
    let request_id = url.trim_start_matches("/v1/node-join/status/");
    if request_id.is_empty() {
        return json_err(400, "request_id required");
    }

    let conn = match open_db() {
        Ok(c) => c,
        Err(e) => return json_err(500, &format!("db error: {e}")),
    };
    let _ = sweep_expired(&conn);

    match get_state_by_id(&conn, request_id) {
        Ok(Some(state)) => json_ok(serde_json::to_value(StatusResponseBody { state }).unwrap()),
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
        Ok(Some((request_id, node_id, wireguard_pubkey, bottom, arch))) => {
            if let Err(e) =
                register_approved_node(&node_id, &wireguard_pubkey, &bottom, &arch, &request_id)
            {
                return json_err(500, &format!("registry error: {e}"));
            }
            if let Err(e) = set_state(&conn, &request_id, "approved") {
                return json_err(500, &format!("state update error: {e}"));
            }
            eprintln!("ppn-pairing: approved {node_id}  ({bottom}/{arch})");
            json_ok(json!({"status": "approved", "node_id": node_id, "bottom": bottom}))
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
        Ok(Some((request_id, node_id, _, _, _))) => {
            if let Err(e) = set_state(&conn, &request_id, "denied") {
                return json_err(500, &format!("state update error: {e}"));
            }
            eprintln!("ppn-pairing: denied {node_id}");
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
                .map(|(id, code, node_id, bottom, created)| {
                    json!({
                        "request_id": id,
                        "code": code,
                        "node_id": node_id,
                        "bottom": bottom,
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
    eprintln!("ppn-pairing: listening on {addr}");

    for mut request in server.incoming_requests() {
        let method = request.method().to_string();
        let url = request.url().to_string();

        let resp: BoxResp = match (method.as_str(), url.as_str()) {
            ("POST", "/v1/node-join/request") => handle_join_request(&mut request),
            ("POST", "/v1/node-join/approve") => handle_approve(&mut request),
            ("POST", "/v1/node-join/deny") => handle_deny(&mut request),
            ("GET", "/v1/node-join/pending") => handle_pending(),
            _ if method == "GET" && url.starts_with("/v1/node-join/status/") => handle_status(&url),
            _ => json_err(404, "not found"),
        };

        let _ = request.respond(resp);
    }

    Ok(())
}

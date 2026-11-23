//! Study Library account server: sign-up with email + password (confirmed by an emailed code),
//! Google sign-in, sessions, and sync of personal study data between a user's devices.
//!
//! Configuration (environment variables):
//!   BIND                  address to listen on (default 127.0.0.1:5764; put HTTPS in front with a reverse proxy)
//!   DB_PATH               SQLite file (default ./accounts.db)
//!   SMTP_HOST, SMTP_PORT, SMTP_USER, SMTP_PASS, SMTP_FROM
//!                         mail server for the codes; when SMTP_HOST is unset, codes are printed to the log
//!   GOOGLE_CLIENT_ID, GOOGLE_CLIENT_SECRET
//!                         an OAuth client of type "Desktop app"; Google sign-in is off when unset

use axum::extract::{ConnectInfo, DefaultBodyLimit, State};
use std::net::{IpAddr, SocketAddr};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};

const CODE_TTL_SECS: i64 = 15 * 60;
const CODE_MAX_ATTEMPTS: i64 = 5;
const CODE_RESEND_SECS: i64 = 60;
const MAX_CHANGES: usize = 20_000;

struct Config {
    smtp_host: Option<String>,
    smtp_port: Option<u16>,
    smtp_user: String,
    smtp_pass: String,
    smtp_from: String,
    google_id: Option<String>,
    google_secret: Option<String>,
}

struct AppState {
    db: Mutex<Connection>,
    cfg: Config,
    http: reqwest::Client,
    /// sign-in requests per client address: (window start, count)
    limits: Mutex<std::collections::HashMap<String, (i64, u32)>>,
}

const AUTH_WINDOW_SECS: i64 = 10 * 60;
const AUTH_MAX_PER_WINDOW: u32 = 30;

/// Limit sign-in, sign-up, code and password-reset requests per client address, which slows
/// down password guessing. When the connection comes from a private or loopback address (the
/// Caddy container, or a proxy on the same machine) the client address is taken from the
/// X-Forwarded-For entry that proxy added; otherwise the connection's own address is used, so a
/// client talking to the server directly cannot fake it.
fn client_ip(peer: SocketAddr, headers: &HeaderMap) -> String {
    let p = peer.ip().to_canonical();
    let behind_proxy = match p {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private(),
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_unique_local(),
    };
    if behind_proxy {
        let fwd = headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.rsplit(',').next())
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());
        if let Some(ip) = fwd {
            return ip;
        }
    }
    p.to_string()
}

fn rate_limit(st: &AppState, peer: SocketAddr, headers: &HeaderMap) -> Result<(), ApiError> {
    let ip = client_ip(peer, headers);
    let mut m = st.limits.lock().unwrap();
    let t = now();
    if m.len() > 50_000 {
        m.retain(|_, (start, _)| t - *start < AUTH_WINDOW_SECS);
    }
    let e = m.entry(ip).or_insert((t, 0));
    if t - e.0 >= AUTH_WINDOW_SECS {
        *e = (t, 0);
    }
    e.1 += 1;
    if e.1 > AUTH_MAX_PER_WINDOW {
        return Err(ApiError(StatusCode::TOO_MANY_REQUESTS, "Too many attempts. Please wait a few minutes and try again.".into(), "rate_limited"));
    }
    Ok(())
}

type Shared = Arc<AppState>;

// ---------------------------------------------------------------- errors

struct ApiError(StatusCode, String, &'static str);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1, "code": self.2 }))).into_response()
    }
}

fn bad(msg: &str) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.into(), "bad_request")
}

fn internal<E: std::fmt::Display>(e: E) -> ApiError {
    eprintln!("internal error: {e}");
    ApiError(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong on the server.".into(), "internal")
}

type ApiResult<T> = Result<Json<T>, ApiError>;

// ---------------------------------------------------------------- helpers

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn now_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

fn sha256_hex(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))
}

fn random_token() -> String {
    use rand::RngCore;
    let mut b = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut b);
    hex::encode(b)
}

fn random_code() -> String {
    use rand::Rng;
    format!("{:06}", rand::rngs::OsRng.gen_range(0..1_000_000))
}

fn normalize_email(e: &str) -> Result<String, ApiError> {
    let e = e.trim().to_lowercase();
    let ok = e.len() <= 254
        && e.split_once('@').map(|(a, d)| !a.is_empty() && d.contains('.') && !d.starts_with('.') && !d.ends_with('.')).unwrap_or(false)
        && !e.contains(char::is_whitespace);
    if ok { Ok(e) } else { Err(bad("Please enter a valid email address.")) }
}

fn check_password(p: &str) -> Result<(), ApiError> {
    if p.chars().count() < 8 {
        return Err(bad("The password must have at least 8 characters."));
    }
    if p.len() > 256 {
        return Err(bad("The password is too long."));
    }
    Ok(())
}

fn hash_password(p: &str) -> Result<String, ApiError> {
    use argon2::password_hash::{rand_core::OsRng, PasswordHasher, SaltString};
    let salt = SaltString::generate(&mut OsRng);
    argon2::Argon2::default().hash_password(p.as_bytes(), &salt).map(|h| h.to_string()).map_err(internal)
}

fn verify_password(p: &str, hash: &str) -> bool {
    use argon2::password_hash::{PasswordHash, PasswordVerifier};
    PasswordHash::new(hash).map(|h| argon2::Argon2::default().verify_password(p.as_bytes(), &h).is_ok()).unwrap_or(false)
}

#[derive(Serialize)]
struct UserOut {
    id: i64,
    email: String,
}

#[derive(Serialize)]
struct SessionOut {
    token: String,
    user: UserOut,
}

fn new_session(db: &Connection, user_id: i64, device: &str) -> Result<SessionOut, ApiError> {
    let token = random_token();
    db.execute(
        "INSERT INTO sessions(token_hash,user_id,device,created,last_used) VALUES(?1,?2,?3,?4,?4)",
        params![sha256_hex(&token), user_id, device.chars().take(100).collect::<String>(), now_iso()],
    )
    .map_err(internal)?;
    let email: String = db.query_row("SELECT email FROM users WHERE id=?1", params![user_id], |r| r.get(0)).map_err(internal)?;
    Ok(SessionOut { token, user: UserOut { id: user_id, email } })
}

/// The signed-in user from the "Authorization: Bearer <token>" header.
fn auth(st: &AppState, headers: &HeaderMap) -> Result<i64, ApiError> {
    let tok = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(ApiError(StatusCode::UNAUTHORIZED, "Please sign in.".into(), "unauthorized"))?;
    let db = st.db.lock().unwrap();
    let h = sha256_hex(tok.trim());
    let uid: Option<i64> = db.query_row("SELECT user_id FROM sessions WHERE token_hash=?1", params![h], |r| r.get(0)).optional().map_err(internal)?;
    let uid = uid.ok_or(ApiError(StatusCode::UNAUTHORIZED, "You were signed out. Please sign in again.".into(), "unauthorized"))?;
    let _ = db.execute("UPDATE sessions SET last_used=?1 WHERE token_hash=?2", params![now_iso(), h]);
    Ok(uid)
}

// ---------------------------------------------------------------- email codes

/// Create a code for (email, purpose) and send it. Respects the resend interval.
async fn issue_code(st: &AppState, email: &str, purpose: &str) -> Result<(), ApiError> {
    let code = random_code();
    {
        let db = st.db.lock().unwrap();
        let last: Option<i64> =
            db.query_row("SELECT sent FROM codes WHERE email=?1 AND purpose=?2", params![email, purpose], |r| r.get(0)).optional().map_err(internal)?;
        if let Some(t) = last {
            if now() - t < CODE_RESEND_SECS {
                return Err(ApiError(StatusCode::TOO_MANY_REQUESTS, "A code was just sent. Please wait a minute before asking for another.".into(), "too_soon"));
            }
        }
        db.execute(
            "INSERT OR REPLACE INTO codes(email,purpose,code_hash,expires,attempts,sent) VALUES(?1,?2,?3,?4,0,?5)",
            params![email, purpose, sha256_hex(&code), now() + CODE_TTL_SECS, now()],
        )
        .map_err(internal)?;
    }
    let (subject, line) = match purpose {
        "reset" => ("Reset your Study Library password", "Use this code to choose a new password:"),
        _ => ("Confirm your Study Library account", "Use this code to confirm your email address:"),
    };
    let body = format!("{line}\n\n    {code}\n\nThe code is valid for 15 minutes. If you didn't ask for it, you can ignore this email.\n");
    send_mail(&st.cfg, email, subject, &body).await
}

async fn send_mail(cfg: &Config, to: &str, subject: &str, body: &str) -> Result<(), ApiError> {
    use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
    let Some(host) = &cfg.smtp_host else {
        eprintln!("[mail not configured] to {to}: {subject}\n{body}");
        return Ok(());
    };
    let msg = Message::builder()
        .from(cfg.smtp_from.parse().map_err(internal)?)
        .to(to.parse().map_err(|_| bad("Please enter a valid email address."))?)
        .subject(subject)
        .body(body.to_string())
        .map_err(internal)?;
    // port 587 (and 25) upgrade a plain connection with STARTTLS; 465 (the default) is TLS from the start
    let mut b = match cfg.smtp_port {
        Some(587) | Some(25) => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host).map_err(internal)?,
        _ => AsyncSmtpTransport::<Tokio1Executor>::relay(host).map_err(internal)?,
    };
    if let Some(p) = cfg.smtp_port {
        b = b.port(p);
    }
    if !cfg.smtp_user.is_empty() {
        b = b.credentials(lettre::transport::smtp::authentication::Credentials::new(cfg.smtp_user.clone(), cfg.smtp_pass.clone()));
    }
    b.build().send(msg).await.map_err(internal)?;
    Ok(())
}

/// Check a code; consumes it when correct.
fn take_code(db: &Connection, email: &str, purpose: &str, code: &str) -> Result<(), ApiError> {
    let wrong = || ApiError(StatusCode::BAD_REQUEST, "That code isn't right, or it has expired.".into(), "bad_code");
    let row: Option<(String, i64, i64)> = db
        .query_row("SELECT code_hash,expires,attempts FROM codes WHERE email=?1 AND purpose=?2", params![email, purpose], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .optional()
        .map_err(internal)?;
    let Some((h, exp, attempts)) = row else { return Err(wrong()) };
    if exp < now() || attempts >= CODE_MAX_ATTEMPTS {
        return Err(wrong());
    }
    if sha256_hex(code.trim()) != h {
        let _ = db.execute("UPDATE codes SET attempts=attempts+1 WHERE email=?1 AND purpose=?2", params![email, purpose]);
        return Err(wrong());
    }
    let _ = db.execute("DELETE FROM codes WHERE email=?1 AND purpose=?2", params![email, purpose]);
    Ok(())
}

// ---------------------------------------------------------------- handlers: email accounts

#[derive(Deserialize)]
struct Credentials {
    email: String,
    password: String,
    #[serde(default)]
    device: String,
}

#[derive(Deserialize)]
struct EmailOnly {
    email: String,
}

#[derive(Deserialize)]
struct CodeReq {
    email: String,
    code: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    device: String,
}

async fn config(State(st): State<Shared>) -> Json<Value> {
    Json(json!({ "google_client_id": st.cfg.google_id, "version": 1 }))
}

async fn signup(State(st): State<Shared>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap, Json(r): Json<Credentials>) -> ApiResult<Value> {
    rate_limit(&st, peer, &headers)?;
    let email = normalize_email(&r.email)?;
    check_password(&r.password)?;
    let hash = hash_password(&r.password)?;
    {
        let db = st.db.lock().unwrap();
        let existing: Option<(i64, bool)> =
            db.query_row("SELECT id,verified FROM users WHERE email=?1", params![email], |r| Ok((r.get(0)?, r.get(1)?))).optional().map_err(internal)?;
        match existing {
            Some((_, true)) => {
                return Err(ApiError(StatusCode::CONFLICT, "An account with this email already exists. Sign in instead.".into(), "exists"));
            }
            Some((id, false)) => {
                db.execute("UPDATE users SET password_hash=?1 WHERE id=?2", params![hash, id]).map_err(internal)?;
            }
            None => {
                db.execute("INSERT INTO users(email,password_hash,verified,created) VALUES(?1,?2,0,?3)", params![email, hash, now_iso()]).map_err(internal)?;
            }
        }
    }
    match issue_code(&st, &email, "verify").await {
        Ok(()) | Err(ApiError(StatusCode::TOO_MANY_REQUESTS, ..)) => Ok(Json(json!({ "ok": true, "next": "verify" }))),
        Err(e) => Err(e),
    }
}

async fn resend(State(st): State<Shared>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap, Json(r): Json<EmailOnly>) -> ApiResult<Value> {
    rate_limit(&st, peer, &headers)?;
    let email = normalize_email(&r.email)?;
    let unverified = {
        let db = st.db.lock().unwrap();
        db.query_row("SELECT 1 FROM users WHERE email=?1 AND verified=0", params![email], |_| Ok(())).optional().map_err(internal)?.is_some()
    };
    if unverified {
        issue_code(&st, &email, "verify").await?;
    }
    Ok(Json(json!({ "ok": true })))
}

async fn verify(State(st): State<Shared>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap, Json(r): Json<CodeReq>) -> ApiResult<SessionOut> {
    rate_limit(&st, peer, &headers)?;
    let email = normalize_email(&r.email)?;
    let db = st.db.lock().unwrap();
    take_code(&db, &email, "verify", &r.code)?;
    let id: i64 = db.query_row("SELECT id FROM users WHERE email=?1", params![email], |r| r.get(0)).map_err(internal)?;
    db.execute("UPDATE users SET verified=1 WHERE id=?1", params![id]).map_err(internal)?;
    Ok(Json(new_session(&db, id, &r.device)?))
}

async fn login(State(st): State<Shared>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap, Json(r): Json<Credentials>) -> Result<Json<SessionOut>, ApiError> {
    rate_limit(&st, peer, &headers)?;
    let email = normalize_email(&r.email)?;
    let wrong = || ApiError(StatusCode::UNAUTHORIZED, "Wrong email or password.".into(), "bad_login");
    let row: Option<(i64, Option<String>, bool)> = {
        let db = st.db.lock().unwrap();
        db.query_row("SELECT id,password_hash,verified FROM users WHERE email=?1", params![email], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional()
            .map_err(internal)?
    };
    let Some((id, Some(hash), verified)) = row else {
        // spend similar time as a real check so timing doesn't reveal which emails exist
        let _ = verify_password(&r.password, "$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHQ$RdescudvJCsgt3ub+b+dWRWJTmaaJObG");
        return Err(wrong());
    };
    if !verify_password(&r.password, &hash) {
        return Err(wrong());
    }
    if !verified {
        let _ = issue_code(&st, &email, "verify").await;
        return Err(ApiError(StatusCode::FORBIDDEN, "Please confirm your email address first. We sent you a code.".into(), "unverified"));
    }
    let db = st.db.lock().unwrap();
    Ok(Json(new_session(&db, id, &r.device)?))
}

async fn forgot(State(st): State<Shared>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap, Json(r): Json<EmailOnly>) -> ApiResult<Value> {
    rate_limit(&st, peer, &headers)?;
    let email = normalize_email(&r.email)?;
    let exists = {
        let db = st.db.lock().unwrap();
        db.query_row("SELECT 1 FROM users WHERE email=?1", params![email], |_| Ok(())).optional().map_err(internal)?.is_some()
    };
    if exists {
        match issue_code(&st, &email, "reset").await {
            Ok(()) | Err(ApiError(StatusCode::TOO_MANY_REQUESTS, ..)) => {}
            Err(e) => return Err(e),
        }
    }
    // same answer either way, so this can't be used to find out who has an account
    Ok(Json(json!({ "ok": true })))
}

async fn reset(State(st): State<Shared>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap, Json(r): Json<CodeReq>) -> ApiResult<SessionOut> {
    rate_limit(&st, peer, &headers)?;
    let email = normalize_email(&r.email)?;
    check_password(&r.password)?;
    let hash = hash_password(&r.password)?;
    let db = st.db.lock().unwrap();
    take_code(&db, &email, "reset", &r.code)?;
    let id: i64 = db.query_row("SELECT id FROM users WHERE email=?1", params![email], |r| r.get(0)).map_err(internal)?;
    db.execute("UPDATE users SET password_hash=?1, verified=1 WHERE id=?2", params![hash, id]).map_err(internal)?;
    // a new password signs out every device
    db.execute("DELETE FROM sessions WHERE user_id=?1", params![id]).map_err(internal)?;
    Ok(Json(new_session(&db, id, &r.device)?))
}

// ---------------------------------------------------------------- handlers: Google

#[derive(Deserialize)]
struct GoogleReq {
    code: String,
    code_verifier: String,
    redirect_uri: String,
    #[serde(default)]
    device: String,
}

async fn google(State(st): State<Shared>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap, Json(r): Json<GoogleReq>) -> ApiResult<SessionOut> {
    rate_limit(&st, peer, &headers)?;
    let (Some(id), Some(secret)) = (&st.cfg.google_id, &st.cfg.google_secret) else {
        return Err(ApiError(StatusCode::NOT_IMPLEMENTED, "Google sign-in isn't set up on this server.".into(), "unavailable"));
    };
    // desktop apps receive the answer on their own computer only
    if !(r.redirect_uri.starts_with("http://127.0.0.1:") || r.redirect_uri.starts_with("http://localhost:")) {
        return Err(bad("Unexpected redirect address."));
    }
    let resp = st
        .http
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("code", r.code.as_str()),
            ("client_id", id.as_str()),
            ("client_secret", secret.as_str()),
            ("redirect_uri", r.redirect_uri.as_str()),
            ("grant_type", "authorization_code"),
            ("code_verifier", r.code_verifier.as_str()),
        ])
        .send()
        .await
        .map_err(internal)?;
    let ok = resp.status().is_success();
    let v: Value = resp.json().await.map_err(internal)?;
    if !ok {
        eprintln!("google token exchange failed: {v}");
        return Err(ApiError(StatusCode::UNAUTHORIZED, "Google sign-in didn't complete. Please try again.".into(), "google_failed"));
    }
    // The ID token came straight from Google's token endpoint over TLS, so its claims can be
    // read without checking the signature (Google's documented guidance); the audience is still checked.
    let idt = v.get("id_token").and_then(|x| x.as_str()).ok_or_else(|| internal("no id_token"))?;
    let payload = idt.split('.').nth(1).ok_or_else(|| internal("bad id_token"))?;
    use base64::Engine;
    let claims: Value = serde_json::from_slice(&base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).map_err(internal)?)
        .map_err(internal)?;
    let s = |k: &str| claims.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let iss = s("iss");
    if s("aud") != *id || !(iss == "accounts.google.com" || iss == "https://accounts.google.com") {
        return Err(internal("id_token audience/issuer mismatch"));
    }
    let verified = claims.get("email_verified").map(|x| x.as_bool() == Some(true) || x.as_str() == Some("true")).unwrap_or(false);
    let sub = s("sub");
    let email = s("email").to_lowercase();
    if sub.is_empty() || email.is_empty() || !verified {
        return Err(ApiError(StatusCode::UNAUTHORIZED, "Your Google account has no confirmed email address.".into(), "google_failed"));
    }
    let db = st.db.lock().unwrap();
    let by_sub: Option<i64> = db.query_row("SELECT id FROM users WHERE google_sub=?1", params![sub], |r| r.get(0)).optional().map_err(internal)?;
    let uid = match by_sub {
        Some(u) => u,
        None => {
            let by_email: Option<i64> = db.query_row("SELECT id FROM users WHERE email=?1", params![email], |r| r.get(0)).optional().map_err(internal)?;
            match by_email {
                // Google confirmed this address belongs to the person, so link the accounts
                Some(u) => {
                    db.execute("UPDATE users SET google_sub=?1, verified=1 WHERE id=?2", params![sub, u]).map_err(internal)?;
                    u
                }
                None => {
                    db.execute("INSERT INTO users(email,verified,google_sub,created) VALUES(?1,1,?2,?3)", params![email, sub, now_iso()]).map_err(internal)?;
                    db.last_insert_rowid()
                }
            }
        }
    };
    Ok(Json(new_session(&db, uid, &r.device)?))
}

// ---------------------------------------------------------------- handlers: session / account

async fn me(State(st): State<Shared>, headers: HeaderMap) -> ApiResult<UserOut> {
    let uid = auth(&st, &headers)?;
    let db = st.db.lock().unwrap();
    let email: String = db.query_row("SELECT email FROM users WHERE id=?1", params![uid], |r| r.get(0)).map_err(internal)?;
    Ok(Json(UserOut { id: uid, email }))
}

async fn logout(State(st): State<Shared>, headers: HeaderMap) -> ApiResult<Value> {
    if let Some(tok) = headers.get("authorization").and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer ")) {
        let db = st.db.lock().unwrap();
        let _ = db.execute("DELETE FROM sessions WHERE token_hash=?1", params![sha256_hex(tok.trim())]);
    }
    Ok(Json(json!({ "ok": true })))
}

async fn delete_account(State(st): State<Shared>, headers: HeaderMap) -> ApiResult<Value> {
    let uid = auth(&st, &headers)?;
    let db = st.db.lock().unwrap();
    db.execute_batch("BEGIN").map_err(internal)?;
    for sql in ["DELETE FROM records WHERE user_id=?1", "DELETE FROM sessions WHERE user_id=?1", "DELETE FROM users WHERE id=?1"] {
        db.execute(sql, params![uid]).map_err(internal)?;
    }
    db.execute_batch("COMMIT").map_err(internal)?;
    Ok(Json(json!({ "ok": true })))
}

// ---------------------------------------------------------------- handlers: sync

#[derive(Deserialize, Serialize)]
struct Change {
    kind: String,
    key: String,
    #[serde(default)]
    data: Value,
    #[serde(default)]
    deleted: bool,
}

#[derive(Deserialize)]
struct SyncReq {
    since: i64,
    #[serde(default)]
    device: String,
    #[serde(default)]
    changes: Vec<Change>,
}

#[derive(Serialize)]
struct SyncResp {
    seq: i64,
    changes: Vec<Change>,
}

/// Store the changes this device sends (each gets the next sequence number, last write wins),
/// then return every record changed after `since`.
async fn sync(State(st): State<Shared>, headers: HeaderMap, Json(r): Json<SyncReq>) -> ApiResult<SyncResp> {
    let uid = auth(&st, &headers)?;
    if r.changes.len() > MAX_CHANGES {
        return Err(bad("Too many changes in one request."));
    }
    let mut db = st.db.lock().unwrap();
    let tx = db.transaction().map_err(internal)?;
    let mut seq: i64 = tx.query_row("SELECT seq FROM users WHERE id=?1", params![uid], |r| r.get(0)).map_err(internal)?;
    let t = now_iso();
    for c in &r.changes {
        if c.kind.is_empty() || c.kind.len() > 32 || c.key.is_empty() || c.key.len() > 512 {
            continue;
        }
        seq += 1;
        let data = if c.deleted { String::new() } else { c.data.to_string() };
        tx.execute(
            "INSERT INTO records(user_id,kind,key,data,deleted,seq,modified,device) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)
             ON CONFLICT(user_id,kind,key) DO UPDATE SET data=excluded.data, deleted=excluded.deleted, seq=excluded.seq,
               modified=excluded.modified, device=excluded.device",
            params![uid, c.kind, c.key, data, c.deleted, seq, t, r.device],
        )
        .map_err(internal)?;
    }
    tx.execute("UPDATE users SET seq=?1 WHERE id=?2", params![seq, uid]).map_err(internal)?;
    let changes = {
        let mut q = tx.prepare("SELECT kind,key,data,deleted FROM records WHERE user_id=?1 AND seq>?2 ORDER BY seq").map_err(internal)?;
        let rows = q
            .query_map(params![uid, r.since], |row| {
                let data: String = row.get(2)?;
                Ok(Change {
                    kind: row.get(0)?,
                    key: row.get(1)?,
                    data: serde_json::from_str(&data).unwrap_or(Value::Null),
                    deleted: row.get(3)?,
                })
            })
            .map_err(internal)?;
        rows.filter_map(|x| x.ok()).collect::<Vec<_>>()
    };
    tx.commit().map_err(internal)?;
    Ok(Json(SyncResp { seq, changes }))
}

// ---------------------------------------------------------------- startup

const SCHEMA: &str = "
PRAGMA journal_mode=WAL;
CREATE TABLE IF NOT EXISTS users(id INTEGER PRIMARY KEY, email TEXT UNIQUE NOT NULL, password_hash TEXT, verified INTEGER NOT NULL DEFAULT 0,
    google_sub TEXT UNIQUE, seq INTEGER NOT NULL DEFAULT 0, created TEXT);
CREATE TABLE IF NOT EXISTS codes(email TEXT NOT NULL, purpose TEXT NOT NULL, code_hash TEXT NOT NULL, expires INTEGER NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0, sent INTEGER NOT NULL, PRIMARY KEY(email, purpose));
CREATE TABLE IF NOT EXISTS sessions(token_hash TEXT PRIMARY KEY, user_id INTEGER NOT NULL, device TEXT, created TEXT, last_used TEXT);
CREATE TABLE IF NOT EXISTS records(user_id INTEGER NOT NULL, kind TEXT NOT NULL, key TEXT NOT NULL, data TEXT, deleted INTEGER NOT NULL DEFAULT 0,
    seq INTEGER NOT NULL, modified TEXT, device TEXT, PRIMARY KEY(user_id, kind, key));
CREATE INDEX IF NOT EXISTS ix_records_seq ON records(user_id, seq);
CREATE INDEX IF NOT EXISTS ix_sessions_user ON sessions(user_id);
";

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.trim().is_empty())
}

#[tokio::main]
async fn main() {
    let db_path = env("DB_PATH").unwrap_or_else(|| "accounts.db".into());
    let db = Connection::open(&db_path).expect("cannot open the database");
    db.execute_batch(SCHEMA).expect("cannot create the tables");
    let cfg = Config {
        smtp_host: env("SMTP_HOST"),
        smtp_port: env("SMTP_PORT").and_then(|p| p.parse().ok()),
        smtp_user: env("SMTP_USER").unwrap_or_default(),
        smtp_pass: env("SMTP_PASS").unwrap_or_default(),
        smtp_from: env("SMTP_FROM").unwrap_or_else(|| "Study Library <no-reply@localhost>".into()),
        google_id: env("GOOGLE_CLIENT_ID"),
        google_secret: env("GOOGLE_CLIENT_SECRET"),
    };
    if cfg.smtp_host.is_none() {
        eprintln!("SMTP_HOST is not set: confirmation codes will be printed here instead of emailed");
    }
    if cfg.google_id.is_none() || cfg.google_secret.is_none() {
        eprintln!("GOOGLE_CLIENT_ID / GOOGLE_CLIENT_SECRET not set: Google sign-in is off");
    }
    let st: Shared = Arc::new(AppState { db: Mutex::new(db), cfg, http: reqwest::Client::new(), limits: Mutex::new(Default::default()) });
    let app = Router::new()
        .route("/v1/config", get(config))
        .route("/v1/auth/signup", post(signup))
        .route("/v1/auth/resend", post(resend))
        .route("/v1/auth/verify", post(verify))
        .route("/v1/auth/login", post(login))
        .route("/v1/auth/forgot", post(forgot))
        .route("/v1/auth/reset", post(reset))
        .route("/v1/auth/google", post(google))
        .route("/v1/auth/logout", post(logout))
        .route("/v1/me", get(me))
        .route("/v1/account", delete(delete_account))
        .route("/v1/sync", post(sync))
        .layer(DefaultBodyLimit::max(64 * 1024 * 1024))
        .with_state(st);
    let bind = env("BIND").unwrap_or_else(|| "127.0.0.1:5764".into());
    let listener = tokio::net::TcpListener::bind(&bind).await.expect("cannot listen on BIND");
    eprintln!("Study Library account server listening on http://{bind}");
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await.unwrap();
}

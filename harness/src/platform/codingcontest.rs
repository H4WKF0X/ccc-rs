//! Client for codingcontest.org (the platform since 2026; CatCoder is gone).
//!
//! Reverse-engineered from a training run (docs/PLATFORM.md):
//!
//! 1. Portal `https://codingcontest.org`, authenticated by the browser's cookies (Spring-style XSRF:
//!    the `XSRF-TOKEN` cookie is echoed in an `X-XSRF-TOKEN` header).
//!    - `GET  /api/contests/{name}`         -> `{slug, gameBaseUrl, ...}`
//!    - `GET  /api/contests/my-registrations` -> `[{contestSlug, contestName, status, ...}]`
//!    - `POST /api/game-token {contestSlug}` -> `{token}`
//! 2. The game app at `gameBaseUrl`, authenticated by `Authorization: <token>` (no scheme word) and
//!    `x-ccc-slug: <contest slug>`.
//!    - `GET  /api/contestant/contestant-info`        -> progress and submissions
//!    - `GET  /api/contestant/level/{L}/files`        -> `{url}`: signed link to the level zip
//!    - `POST /api/contestant/submit-{L}-{fileId}`    -> multipart field `solution`;
//!      returns `{evaluation: {isCorrect, cases: [{caseMessage, ...}]}}`
//!
//! Credentials come from `ccc.local.toml` (git-ignored): either `cookie` (the whole Cookie header
//! sent to codingcontest.org) or `game_token` + `game_url` + `contest_slug` copied from a game request.

use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::time::Duration;

use serde_json::Value;

use crate::config::Config;
use crate::ingest;
use crate::layout::{Workspace, file_id, file_name};

const PORTAL: &str = "https://codingcontest.org";
const MAX_DOWNLOAD: u64 = 1 << 30;

const MISSING_AUTH: &str = "no codingcontest.org credentials. In ccc.local.toml set either\n  \
    cookie = \"...\"  (DevTools -> Network -> any codingcontest.org/api request -> Request Headers -> Cookie)\n\
    or game_token + game_url + contest_slug (from a request to the game, see docs/PLATFORM.md).";

/// An authenticated session with one contest's game app.
struct Game {
    agent: ureq::Agent,
    base: String,
    token: String,
    slug: String,
}

impl Game {
    fn connect(cfg: &Config) -> Result<Game, String> {
        let agent = agent();
        if let Some(token) = cfg.get("game_token") {
            let base = cfg.get("game_url").ok_or("game_token is set, so game_url must be too")?;
            let slug = cfg.get("contest_slug").ok_or("game_token is set, so contest_slug must be too")?;
            return Ok(Game { agent, base: base.trim_end_matches('/').into(), token: token.into(), slug: slug.into() });
        }
        let portal = Portal { agent: &agent, cookie: cfg.get("cookie").ok_or(MISSING_AUTH)? };
        let contest = portal.find_contest(cfg.get("contest"))?;
        let slug = str_field(&contest, "slug")?;
        let base = str_field(&contest, "gameBaseUrl")?.trim_end_matches('/').to_owned();
        let token = str_field(&portal.post_json("/api/game-token", &serde_json::json!({ "contestSlug": slug }))?, "token")?;
        Ok(Game { agent, base, token, slug })
    }

    fn request(&self, method: &str, path: &str) -> String {
        format!("{method} {}{path}", self.base)
    }

    fn get_json(&self, path: &str) -> Result<Value, String> {
        let resp = self
            .agent
            .get(format!("{}{path}", self.base))
            .header("Authorization", &self.token)
            .header("x-ccc-slug", &self.slug)
            .call();
        json_body(resp, &self.request("GET", path))
    }

    fn progress(&self) -> Result<Value, String> {
        self.get_json("/api/contestant/contestant-info")
    }

    fn level_zip(&self, level: u32) -> Result<Vec<u8>, String> {
        let link = self.get_json(&format!("/api/contestant/level/{level}/files"))?;
        let url = str_field(&link, "url")?;
        let mut resp = self.agent.get(&url).call().map_err(|e| format!("downloading level {level} zip: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("downloading level {level} zip: HTTP {}", resp.status()));
        }
        resp.body_mut()
            .with_config()
            .limit(MAX_DOWNLOAD)
            .read_to_vec()
            .map_err(|e| format!("downloading level {level} zip: {e}"))
    }

    fn submit(&self, level: u32, id: &str, name: &str, content: &[u8]) -> Result<Value, String> {
        let path = format!("/api/contestant/submit-{level}-{id}");
        let boundary = format!("----ccc{}", std::process::id());
        let resp = self
            .agent
            .post(format!("{}{path}", self.base))
            .header("Authorization", &self.token)
            .header("x-ccc-slug", &self.slug)
            .header("Origin", self.base.as_str())
            .content_type(format!("multipart/form-data; boundary={boundary}"))
            .send(&multipart(&boundary, "solution", name, content)[..]);
        json_body(resp, &self.request("POST", &path))
    }
}

/// The portal, authenticated with the browser's cookies.
struct Portal<'a> {
    agent: &'a ureq::Agent,
    cookie: &'a str,
}

impl Portal<'_> {
    fn xsrf(&self) -> &str {
        self.cookie
            .split(';')
            .find_map(|c| c.trim().strip_prefix("XSRF-TOKEN="))
            .unwrap_or("")
    }

    fn get_json(&self, path: &str) -> Result<Value, String> {
        let resp = self.agent.get(format!("{PORTAL}{path}")).header("Cookie", self.cookie).call();
        json_body(resp, &format!("GET {PORTAL}{path}"))
    }

    fn post_json(&self, path: &str, body: &Value) -> Result<Value, String> {
        let resp = self
            .agent
            .post(format!("{PORTAL}{path}"))
            .header("Cookie", self.cookie)
            .header("X-XSRF-TOKEN", self.xsrf())
            .header("Origin", PORTAL)
            .header("Referer", format!("{PORTAL}/"))
            .content_type("application/json")
            .send(body.to_string().as_bytes());
        json_body(resp, &format!("POST {PORTAL}{path}"))
    }

    /// The contest named in ccc.toml (`contest` = the `<name>` in the portal URL
    /// `/contests/<name>/game`), or else the one running contest you are registered for.
    fn find_contest(&self, configured: Option<&str>) -> Result<Value, String> {
        if let Some(name) = configured {
            return self.get_json(&format!("/api/contests/{name}"));
        }
        let regs = self.get_json("/api/contests/my-registrations")?;
        let running: Vec<&Value> = regs
            .as_array()
            .into_iter()
            .flatten()
            .filter(|r| matches!(r["status"].as_str(), Some("RUNNING" | "STATS_FROZEN")))
            .collect();
        let [reg] = running[..] else {
            return Err(format!(
                "{} running contests in your registrations; set contest = \"<name from the portal URL \
                 /contests/<name>/game>\" in ccc.toml",
                running.len()
            ));
        };
        // The portal routes by name or slug; try both.
        ["contestSlug", "contestName"]
            .iter()
            .filter_map(|key| reg[*key].as_str())
            .find_map(|key| self.get_json(&format!("/api/contests/{key}")).ok())
            .ok_or_else(|| "found a running registration but could not load the contest; set contest in ccc.toml".into())
    }
}

// ----------------------------------------------------------------------------- commands

pub fn progress(cfg: &Config) -> Result<bool, String> {
    let game = Game::connect(cfg)?;
    let info = game.progress()?;
    let score = &info["score"]["gameScore"];
    println!("Contest {} on {}", game.slug, game.base);
    println!("  current level: {}   time: {}", score["level"], format_ms(score["timeMs"].as_u64()));
    if let Some(levels) = info["score"]["state"].as_object() {
        for (level, state) in levels {
            let subs = state["submissions"].as_array().map_or(&[][..], Vec::as_slice);
            let correct = subs.iter().filter(|s| s["result"]["correct"].as_bool() == Some(true)).count();
            let passed: Vec<&str> = state["passedFiles"]
                .as_object()
                .into_iter()
                .flatten()
                .filter(|(_, v)| !v.is_null())
                .map(|(k, _)| k.as_str())
                .collect();
            println!("  {level}: {} submission(s), {correct} correct; passed: [{}]", subs.len(), passed.join(", "));
        }
    }
    Ok(true)
}

pub fn fetch(ws: &Workspace, cfg: &Config, level: u32) -> Result<bool, String> {
    let game = Game::connect(cfg)?;
    let bytes = game.level_zip(level)?;
    println!("Downloaded level {level} ({} KB) from {}", bytes.len() / 1024, game.base);
    let mut files = Vec::new();
    ingest::add(format!("level-{level}.zip"), bytes, &mut files)?;
    ingest::store(ws, &files, Some(level))
}

pub fn submit(cfg: &Config, level: u32, outputs: &[PathBuf], yes: bool) -> Result<bool, String> {
    let mut uploads = Vec::new();
    for out in outputs {
        let name = file_name(out);
        let id = file_id(&name).ok_or_else(|| format!("can't tell the platform file id from {name:?}"))?;
        uploads.push((id, name, out));
    }
    let game = Game::connect(cfg)?;
    println!("About to submit {} file(s) for level {level} to {} ({}):", uploads.len(), game.base, game.slug);
    for (id, name, _) in &uploads {
        println!("  {id:<10} {name}");
    }
    if !yes && !confirm()? {
        println!("Aborted. Nothing uploaded.");
        return Ok(false);
    }
    let mut all_correct = true;
    for (id, name, path) in &uploads {
        let content = std::fs::read(path).map_err(|e| format!("could not read {name}: {e}"))?;
        let result = game.submit(level, id, name, &content)?;
        let eval = &result["evaluation"];
        let correct = eval["isCorrect"].as_bool() == Some(true);
        all_correct &= correct;
        println!("{} {id}", if correct { "[CORRECT]" } else { "[WRONG]  " });
        if !correct {
            let cases = eval["cases"].as_array().map_or(&[][..], Vec::as_slice);
            let wrong: Vec<&Value> = cases.iter().filter(|c| c["isCorrect"].as_bool() != Some(true)).collect();
            for case in wrong.iter().take(5) {
                println!(
                    "    lines {}..{}: {} {}",
                    case["startLineNumber"],
                    case["endLineNumberExcl"],
                    case["caseState"].as_str().unwrap_or("?"),
                    case["caseMessage"].as_str().unwrap_or("")
                );
            }
            if wrong.len() > 5 {
                println!("    ... and {} more wrong case(s)", wrong.len() - 5);
            }
            if cases.is_empty() {
                println!("    response: {}", clip(&result.to_string(), 500));
            }
        }
    }
    println!("{}", if all_correct { "All submissions correct." } else { "Some submissions were wrong (see above)." });
    Ok(all_correct)
}

// ----------------------------------------------------------------------------- helpers

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(120)))
        .user_agent("ccc-rs")
        .build()
        .into()
}

fn json_body(resp: Result<ureq::http::Response<ureq::Body>, ureq::Error>, what: &str) -> Result<Value, String> {
    let mut resp = resp.map_err(|e| format!("{what}: {e}"))?;
    let status = resp.status();
    let text = resp.body_mut().read_to_string().map_err(|e| format!("{what}: reading response: {e}"))?;
    if !status.is_success() {
        let hint = if matches!(status.as_u16(), 401 | 403) {
            "\n  -> not authorised: the cookie or game token has probably expired; copy a fresh one (docs/PLATFORM.md)"
        } else {
            ""
        };
        return Err(format!("{what}: HTTP {status}: {}{hint}", clip(&text, 300)));
    }
    serde_json::from_str(&text).map_err(|e| format!("{what}: response is not JSON ({e}): {}", clip(&text, 300)))
}

fn str_field(v: &Value, key: &str) -> Result<String, String> {
    v[key]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("response has no {key:?} field: {}", clip(&v.to_string(), 300)))
}

fn multipart(boundary: &str, field: &str, filename: &str, content: &[u8]) -> Vec<u8> {
    let mut body = Vec::with_capacity(content.len() + 256);
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"{field}\"; filename=\"{filename}\"\r\n\
             Content-Type: text/plain\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(content);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    body
}

fn confirm() -> Result<bool, String> {
    print!("Type 'yes' to upload: ");
    std::io::stdout().flush().ok();
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line).map_err(|e| format!("reading confirmation: {e}"))?;
    Ok(line.trim().eq_ignore_ascii_case("yes"))
}

fn format_ms(ms: Option<u64>) -> String {
    ms.map_or("?".into(), |ms| format!("{}:{:02}", ms / 60_000, ms / 1000 % 60))
}

fn clip(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((cut, _)) => format!("{}…", &s[..cut]),
        None => s.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multipart_body_shape() {
        let body = String::from_utf8(multipart("B", "solution", "out_level-1_1-small.txt", b"1\r\n2\r\n")).unwrap();
        assert_eq!(
            body,
            "--B\r\nContent-Disposition: form-data; name=\"solution\"; filename=\"out_level-1_1-small.txt\"\r\n\
             Content-Type: text/plain\r\n\r\n1\r\n2\r\n\r\n--B--\r\n"
        );
    }

    #[test]
    fn xsrf_from_cookie_header() {
        let agent = agent();
        let portal = Portal { agent: &agent, cookie: "SESSION=abc; XSRF-TOKEN=x-123; other=1" };
        assert_eq!(portal.xsrf(), "x-123");
    }
}

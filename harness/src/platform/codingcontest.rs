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
//! Credentials, first match wins:
//! 1. `game_token` + `game_url` + `contest_slug` in the config (copied from a request to the game);
//! 2. `cookie` in the config (the whole Cookie header sent to codingcontest.org);
//! 3. otherwise, read straight from Firefox (log in to codingcontest.org there; see firefox.rs).
//!    Read fresh on every command, so an expired login is fixed by logging in again in the browser.
//!
//! The contest is `contest` from the config, else your one running contest registration, else your
//! active training run.

use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::time::Duration;

use super::firefox;

use serde_json::Value;

use crate::config::Config;
use crate::ingest;
use crate::layout::{Workspace, file_id, file_name};

const PORTAL: &str = "https://codingcontest.org";
const MAX_DOWNLOAD: u64 = 1 << 30;

const LOGIN_HELP: &str = "Log in to codingcontest.org in Firefox, or put cookie = \"...\" in \
    ~/.config/ccc/config.toml (DevTools -> Network -> any codingcontest.org request -> Request Headers -> Cookie).";

/// An authenticated session with one contest's game app.
struct Game {
    agent: ureq::Agent,
    base: String,
    token: String,
    slug: String,
    /// Start, server "now" and duration, when we came through the portal (not with game_token).
    clock: Option<Clock>,
    total_levels: Option<u64>,
    /// Where the credentials came from, for `login`.
    auth: String,
    username: Option<String>,
}

struct Clock {
    start_ms: u64,
    server_now_ms: u64,
    duration_ms: u64,
}

impl Clock {
    fn from_contest(contest: &Value) -> Option<Clock> {
        Some(Clock {
            start_ms: parse_utc_ms(contest["startTime"].as_str()?)?,
            server_now_ms: contest["serverNowMs"].as_u64()?,
            duration_ms: contest["durationMinutes"].as_u64()? * 60_000,
        })
    }

    fn describe(&self) -> String {
        let elapsed = self.server_now_ms.saturating_sub(self.start_ms);
        let left = self.duration_ms.saturating_sub(elapsed);
        format!(
            "{} elapsed, {} left (of {})",
            format_ms(Some(elapsed)),
            format_ms(Some(left)),
            format_ms(Some(self.duration_ms))
        )
    }
}

impl Game {
    fn connect(cfg: &Config) -> Result<Game, String> {
        Game::connect_staged(cfg).map_err(|e| match e {
            Connect::Login(why) | Connect::NoContest { why, .. } | Connect::GameApi { why, .. } => why,
            Connect::NotStarted { contest, start, .. } => format!("{contest} has not started yet (starts {start})"),
        })
    }

    /// Like `connect`, but tells "the login failed" apart from "logged in, but no contest".
    fn connect_staged(cfg: &Config) -> Result<Game, Connect> {
        let agent = agent();
        if let Some(token) = cfg.get("game_token") {
            let base = cfg.get("game_url").ok_or(Connect::Login("game_token is set, so game_url must be too".into()))?;
            let slug = cfg.get("contest_slug").ok_or(Connect::Login("game_token is set, so contest_slug must be too".into()))?;
            return Ok(Game {
                agent,
                base: base.trim_end_matches('/').into(),
                token: token.into(),
                slug: slug.into(),
                clock: None,
                total_levels: None,
                auth: "game_token from config".into(),
                username: None,
            });
        }
        let (cookie, auth) = match cfg.get("cookie") {
            Some(cookie) => (cookie.to_owned(), "cookie from config".to_owned()),
            None => {
                let (cookie, profile) = firefox::cookie_header("codingcontest.org", "SESSION")
                    .map_err(|e| Connect::Login(format!("not logged in: {e}.\n{LOGIN_HELP}")))?;
                (cookie, format!("Firefox profile {profile}"))
            }
        };
        let portal = Portal { agent: &agent, cookie: &cookie };
        let user = match portal.get_json("/api/auth/current-user") {
            Ok(u) => u["username"].as_str().map(str::to_owned),
            Err(e) => return Err(Connect::Login(format!("could not reach codingcontest.org ({auth}): {e}"))),
        };
        let Some(user) = user else {
            return Err(Connect::Login(format!(
                "codingcontest.org did not accept the login ({auth}); it has probably expired.\n{LOGIN_HELP}"
            )));
        };
        // Nothing running: a contest that hasn't started yet is expected, not an error.
        let contest = match portal.find_contest(cfg.get("contest")) {
            Ok(contest) if is_running(&contest) => contest,
            Ok(contest) => return Err(not_running(&user, &contest)),
            Err(why) => {
                return Err(match portal.upcoming_registration() {
                    Some(reg) => not_running(&user, &reg),
                    None => Connect::NoContest { user, why },
                });
            }
        };
        // From here on the contest is running, so a failure means the game API doesn't work for us.
        let name = contest["slug"].as_str().unwrap_or("the contest").to_owned();
        let api_failed = |why: String| Connect::GameApi { user: user.clone(), contest: name.clone(), why };
        let slug = str_field(&contest, "slug").map_err(api_failed)?;
        let base = str_field(&contest, "gameBaseUrl").map_err(api_failed)?.trim_end_matches('/').to_owned();
        let token = portal
            .post_json("/api/game-token", &serde_json::json!({ "contestSlug": slug }))
            .and_then(|t| str_field(&t, "token"))
            .map_err(api_failed)?;
        let username = Some(user);
        Ok(Game {
            agent,
            base,
            token,
            slug,
            clock: Clock::from_contest(&contest),
            total_levels: contest["totalLevels"].as_u64(),
            auth,
            username,
        })
    }

    fn request(&self, method: &str, path: &str) -> String {
        format!("{method} {}{path}", self.base)
    }

    fn get_json(&self, path: &str) -> Result<Value, String> {
        let url = format!("{}{path}", self.base);
        let resp = with_retries(|| {
            self.agent.get(&url).header("Authorization", &self.token).header("x-ccc-slug", &self.slug).call()
        });
        json_body(resp, &self.request("GET", path))
    }

    /// How many levels the game has: from the contest record, else from the game app itself
    /// (`/game/game-info` -> `levelsInfo.count`). `None` if neither says.
    fn levels(&self) -> Option<u64> {
        self.total_levels.or_else(|| self.get_json("/game/game-info").ok()?["levelsInfo"]["count"].as_u64())
    }

    fn progress(&self) -> Result<Value, String> {
        self.get_json("/api/contestant/contestant-info")
    }

    fn level_zip(&self, level: u32) -> Result<Vec<u8>, String> {
        let link = self.get_json(&format!("/api/contestant/level/{level}/files")).map_err(|e| {
            if e.contains("HTTP 404") {
                format!("level {level} is not available: past the game's last level, or not unlocked yet ({e})")
            } else {
                e
            }
        })?;
        let url = str_field(&link, "url")?;
        let mut resp = with_retries(|| self.agent.get(&url).call()).map_err(|e| format!("downloading level {level} zip: {e}"))?;
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
        // Never retried: a resend after a lost response could count as a second (penalised) attempt.
        if let Err(e) = &resp {
            return Err(format!(
                "{}: {e}\n  -> the upload may or may not have arrived; check `cargo ccc progress` before resubmitting",
                self.request("POST", &path)
            ));
        }
        json_body(resp, &self.request("POST", &path))
    }
}

/// Why `connect` failed. Only `GameApi` (the contest runs, but we can't use its API) is a reason to
/// switch to manual mode; `NotStarted` before the start is simply expected.
enum Connect {
    /// The account login (cookie) is missing or rejected.
    Login(String),
    /// Logged in; the contest found is not running yet (or any more).
    NotStarted { user: String, contest: String, start: String, status: String },
    /// Logged in, but no running or upcoming contest at all.
    NoContest { user: String, why: String },
    /// The contest is running, but getting the game token / talking to the game failed.
    GameApi { user: String, contest: String, why: String },
}

fn is_running(contest: &Value) -> bool {
    // Training runs and the real contest both report RUNNING (STATS_FROZEN: leaderboard frozen, still on).
    matches!(contest["status"].as_str(), Some("RUNNING" | "STATS_FROZEN") | None)
}

/// A `NotStarted` for a contest record or registration summary.
fn not_running(user: &str, c: &Value) -> Connect {
    let contest = ["slug", "contestSlug", "contestName", "name"]
        .iter()
        .find_map(|k| c[*k].as_str())
        .unwrap_or("the contest")
        .to_owned();
    let start = c["startTime"].as_str().map_or("at an unknown time".to_owned(), describe_start);
    let status = c["status"].as_str().unwrap_or("?").to_owned();
    Connect::NotStarted { user: user.to_owned(), contest, start, status }
}

/// `2026-10-23T08:00:00Z` -> `2026-10-23 08:00 UTC (in 1 h 12 min)` / `(started 3 min ago)`.
fn describe_start(iso: &str) -> String {
    let when = iso.get(..16).unwrap_or(iso).replace('T', " ");
    let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64);
    match parse_utc_ms(iso) {
        Some(start) => {
            let mins = (start as i64 - now_ms) / 60_000;
            let rel = |m: i64| if m >= 60 { format!("{} h {} min", m / 60, m % 60) } else { format!("{m} min") };
            if mins >= 0 { format!("{when} UTC (in {})", rel(mins)) } else { format!("{when} UTC (started {} ago)", rel(-mins)) }
        }
        None => format!("{when} UTC"),
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
        let url = format!("{PORTAL}{path}");
        let resp = with_retries(|| self.agent.get(&url).header("Cookie", self.cookie).call());
        json_body(resp, &format!("GET {url}"))
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

    /// The contest to play: `contest` from the config (the `<name>` in the portal URL
    /// `/contests/<name>/game`), else your one running registration (a real contest beats a
    /// training run), else your active training run.
    /// The next contest you're registered for that hasn't finished, if any (earliest start first).
    fn upcoming_registration(&self) -> Option<Value> {
        let regs = self.get_json("/api/contests/my-registrations").ok()?;
        regs.as_array()?
            .iter()
            .filter(|r| !matches!(r["status"].as_str(), Some("FINISHED" | "CANCELLED")))
            .min_by_key(|r| r["startTime"].as_str().unwrap_or("~").to_owned())
            .cloned()
    }

    fn find_contest(&self, configured: Option<&str>) -> Result<Value, String> {
        if let Some(name) = configured {
            return self.get_json(&format!("/api/contests/{}", path_segment(name)));
        }
        let regs = self.get_json("/api/contests/my-registrations")?;
        let running: Vec<&Value> = regs
            .as_array()
            .into_iter()
            .flatten()
            .filter(|r| matches!(r["status"].as_str(), Some("RUNNING" | "STATS_FROZEN")))
            .collect();
        let is_training = |r: &&Value| ["contestSlug", "contestName"].iter().any(|k| r[*k].as_str().is_some_and(|v| v.starts_with("training")));
        let real: Vec<&Value> = running.iter().copied().filter(|r| !is_training(r)).collect();
        let preferred = if real.is_empty() { running.clone() } else { real };
        match preferred[..] {
            [reg] => return self.load_contest(reg),
            [] => {}
            _ => {
                return Err(format!(
                    "{} running contests in your registrations; set contest = \"<name from the portal URL \
                     /contests/<name>/game>\" in ccc.toml",
                    preferred.len()
                ));
            }
        }
        // A list of training summaries (an object in older responses); take the most recently started.
        let active = self.get_json("/api/training/active").unwrap_or(Value::Null);
        let latest = match &active {
            Value::Array(runs) => runs.iter().max_by_key(|r| r["startTime"].as_str().unwrap_or("")),
            Value::Object(_) => Some(&active),
            _ => None,
        };
        match latest {
            Some(run) => self.load_contest(run),
            None => Err("no running contest or active training found. Start one in the browser, or set contest = \
                         \"<name from the portal URL /contests/<name>/game>\" in ccc.toml"
                .into()),
        }
    }

    /// Load the full contest record behind a registration or training summary. The portal routes
    /// by name or slug and the summaries name fields differently, so try what's there.
    fn load_contest(&self, summary: &Value) -> Result<Value, String> {
        // Slugs first: registration names contain spaces ("42nd SCHOOL CCC").
        let candidates = [&summary["contestSlug"], &summary["slug"], &summary["contestName"], &summary["name"]]
            .into_iter()
            .chain([&summary["contest"]["slug"], &summary["contest"]["name"]])
            .filter_map(Value::as_str);
        let mut last_err = String::from("no contest name in the summary");
        for key in candidates {
            match self.get_json(&format!("/api/contests/{}", path_segment(key))) {
                Ok(c) if c["gameBaseUrl"].is_string() => return Ok(c),
                Ok(_) => last_err = format!("/api/contests/{key} has no gameBaseUrl"),
                Err(e) => last_err = e,
            }
        }
        Err(format!("could not load the contest ({last_err}); set contest in ccc.toml. Summary: {}", clip(&summary.to_string(), 300)))
    }
}

// ----------------------------------------------------------------------------- commands

/// `cargo ccc login`: one unmistakable state, and what to do next. Only GAME API FAILED (the contest
/// runs but we can't use its API) points to manual mode; before the start, "not started" is expected.
pub fn login(cfg: &Config, switch_to_manual: &str) -> Result<bool, String> {
    let rule = "=".repeat(78);
    let banner = |lines: &[String]| {
        println!("{rule}");
        lines.iter().for_each(|l| println!("{l}"));
        println!("{rule}");
    };
    let game = match Game::connect_staged(cfg) {
        Ok(game) => game,
        Err(Connect::Login(why)) => {
            banner(&[
                format!("ACCOUNT LOGIN FAILED: {why}"),
                "  Log in to codingcontest.org in Firefox, then run `cargo ccc login` again.".into(),
                format!("  If it still fails once the contest runs: {switch_to_manual}"),
            ]);
            return Ok(false);
        }
        Err(Connect::NotStarted { user, contest, start, status }) => {
            banner(&[
                format!("ACCOUNT OK: {user}.  CONTEST NOT RUNNING: {contest} ({status}), starts {start}."),
                "  That's expected before the start. Once it runs: open the game in the browser, then".into(),
                "  `cargo ccc login` again: that checks the game API.".into(),
            ]);
            return Ok(false);
        }
        Err(Connect::NoContest { user, why }) => {
            banner(&[
                format!("ACCOUNT OK: {user}.  NO CONTEST: {why}"),
                "  Training: start a game in the browser. Contest: check your registration, or set".into(),
                "  contest = \"<name>\" in ccc.toml (from the browser URL /contests/<name>/game).".into(),
            ]);
            return Ok(false);
        }
        Err(Connect::GameApi { user, contest, why }) => {
            banner(&[
                format!("GAME API FAILED: {user} @ {contest} is running, but: {why}"),
                format!("  Switch to manual mode now: {switch_to_manual}"),
            ]);
            return Ok(false);
        }
    };
    let level = match game.progress() {
        Ok(info) => current_level(&info),
        Err(why) => {
            banner(&[
                format!("GAME API FAILED: {} is running, but the game app answered: {why}", game.slug),
                format!("  Switch to manual mode now: {switch_to_manual}"),
            ]);
            return Ok(false);
        }
    };
    let total = game.levels().map_or(String::new(), |t| format!(" of {t}"));
    let left = game.clock.as_ref().map_or(String::new(), |c| format!(", {}", c.describe()));
    banner(&[
        format!("LOGIN OK: {} @ {} (login from {})", game.username.as_deref().unwrap_or("?"), game.slug, game.auth),
        format!("  game {}, level {}{total} unlocked{left}", game.base, level.map_or("?".into(), |l| l.to_string())),
        "  Next: start `claude` in this folder and type /level".into(),
    ]);
    Ok(true)
}

pub fn progress(cfg: &Config) -> Result<bool, String> {
    print_progress(&Game::connect(cfg)?)
}

fn current_level(info: &Value) -> Option<u64> {
    info["score"]["gameScore"]["level"].as_u64()
}

fn print_progress(game: &Game) -> Result<bool, String> {
    let info = game.progress()?;
    let score = &info["score"]["gameScore"];
    let levels = game.levels();
    let total = levels.map_or(String::new(), |t| format!(" of {t}"));
    println!("Contest {} on {}", game.slug, game.base);
    if let Some(clock) = &game.clock {
        println!("  contest clock: {}", clock.describe());
    }
    // gameScore.timeMs is the score's time component, not a clock: 0 until a level is passed.
    println!("  current level: {}{total}   score time: {}", score["level"], format_ms(score["timeMs"].as_u64()));
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
    if let Some(total) = game.levels()
        && u64::from(level) > total
    {
        return Err(format!("this game has only {total} levels, so there is no level {level}"));
    }
    let bytes = game.level_zip(level)?;
    println!("Downloaded level {level} ({} KB) from {}", bytes.len() / 1024, game.base);
    let mut files = Vec::new();
    ingest::add(format!("level-{level}.zip"), bytes, &mut files)?;
    ingest::store(ws, cfg, &files, Some(level))
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
        println!("{} {id}{}", if correct { "[CORRECT]" } else { "[WRONG]  " }, extra_fields(&result));
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
    if !all_correct {
        println!("Some submissions were wrong (see above). Each wrong upload costs penalty time: fix, rerun `all`, then resubmit.");
        return Ok(false);
    }
    println!("All submissions correct.");
    let now = game.progress().ok().as_ref().and_then(current_level);
    if let Some(next) = next_step(level, game.levels(), now) {
        println!("{next}");
    }
    Ok(true)
}

/// What to say after level `level` was passed, given the game's level count and the level the game
/// now reports as current. After the last level the game reports one past the end, so the end is
/// checked first.
fn next_step(level: u32, total: Option<u64>, now: Option<u64>) -> Option<String> {
    let level = u64::from(level);
    match (total, now) {
        (Some(total), _) if level >= total => Some(format!("That was the last level ({level} of {total}): the game is complete.")),
        (Some(total), Some(now)) if now > level => Some(format!("Level {now} of {total} is unlocked: next is `cargo ccc fetch {now}`.")),
        (None, Some(now)) if now > level => Some(format!(
            "The game now reports level {now} (couldn't tell how many levels it has; `fetch {now}` fails if that was the last)."
        )),
        _ => None,
    }
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

/// Retry transport failures (not HTTP errors) twice, with a short backoff. Only for idempotent requests.
fn with_retries<T>(mut call: impl FnMut() -> Result<T, ureq::Error>) -> Result<T, ureq::Error> {
    let mut attempt = 0;
    loop {
        match call() {
            // Only network trouble is worth a retry; a bad URL or HTTP status won't change.
            Err(e)
                if attempt < 2
                    && matches!(
                        e,
                        ureq::Error::Io(_) | ureq::Error::Timeout(_) | ureq::Error::HostNotFound | ureq::Error::ConnectionFailed
                    ) =>
            {
                attempt += 1;
                eprintln!("  (network error: {e}; retrying)");
                std::thread::sleep(Duration::from_millis(500 * attempt));
            }
            result => return result,
        }
    }
}

/// Everything in a submit response besides the verdict and the per-case list (e.g. the score of an
/// optimization level), as `  score=26756 rank=12`. Field names aren't documented, so show them all.
fn extra_fields(result: &Value) -> String {
    let mut parts = Vec::new();
    let mut add = |prefix: &str, obj: &Value| {
        for (key, value) in obj.as_object().into_iter().flatten() {
            if matches!(key.as_str(), "evaluation" | "cases" | "isCorrect") {
                continue;
            }
            let shown = match value {
                Value::String(s) => s.clone(),
                Value::Array(_) | Value::Object(_) => clip(&value.to_string(), 80),
                other => other.to_string(),
            };
            parts.push(format!("{prefix}{key}={shown}"));
        }
    };
    add("", result);
    add("", &result["evaluation"]);
    if parts.is_empty() { String::new() } else { format!("  {}", parts.join(" ")) }
}

/// Percent-encode one URL path segment (anything but unreserved characters).
fn path_segment(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn json_body(resp: Result<ureq::http::Response<ureq::Body>, ureq::Error>, what: &str) -> Result<Value, String> {
    let mut resp = resp.map_err(|e| format!("{what}: {e}"))?;
    let status = resp.status();
    let text = resp.body_mut().read_to_string().map_err(|e| format!("{what}: reading response: {e}"))?;
    if !status.is_success() {
        let hint = if matches!(status.as_u16(), 401 | 403) {
            "\n  -> not authorised: the login has probably expired. Log in to codingcontest.org in Firefox again \
             (or refresh cookie/game_token in the config)."
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

/// `h:mm:ss`.
fn format_ms(ms: Option<u64>) -> String {
    ms.map_or("?".into(), |ms| {
        let s = ms / 1000;
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    })
}

/// `2026-10-19T09:16:22.802336Z` -> Unix milliseconds. UTC (`Z`) only, which is what the portal sends.
fn parse_utc_ms(s: &str) -> Option<u64> {
    let s = s.strip_suffix('Z')?;
    let (date, time) = s.split_once('T')?;
    let mut d = date.split('-').map(|p| p.parse::<i64>());
    let (y, m, day) = (d.next()?.ok()?, d.next()?.ok()?, d.next()?.ok()?);
    let (hms, frac) = time.split_once('.').unwrap_or((time, "0"));
    let mut t = hms.split(':').map(|p| p.parse::<i64>());
    let (hh, mm, ss) = (t.next()?.ok()?, t.next()?.ok()?, t.next()?.ok()?);
    let millis: i64 = format!("{frac:0<3}")[..3].parse().ok()?;
    // Days since 1970-01-01 (Howard Hinnant's days_from_civil).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(((days * 24 + hh) * 60 + mm) * 60_000 + ss * 1000 + millis).ok()
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
    fn shows_extra_response_fields() {
        let r = serde_json::json!({"evaluation": {"isCorrect": true, "score": 26756, "cases": []}});
        assert_eq!(extra_fields(&r), "  score=26756");
        assert_eq!(extra_fields(&serde_json::json!({"evaluation": {"isCorrect": true}})), "");
    }

    #[test]
    fn encodes_path_segments() {
        assert_eq!(path_segment("42nd SCHOOL CCC"), "42nd%20SCHOOL%20CCC");
        assert_eq!(path_segment("training-ccc-2025.11-x"), "training-ccc-2025.11-x");
    }

    #[test]
    fn describes_start_times() {
        assert_eq!(describe_start("2020-01-01T00:00:00Z").split(" (").next().unwrap(), "2020-01-01 00:00 UTC");
        assert!(describe_start("2020-01-01T00:00:00Z").contains("started"));
        assert!(describe_start("2999-01-01T08:00:00Z").contains("(in "));
        assert!(is_running(&serde_json::json!({"status": "RUNNING"})));
        assert!(!is_running(&serde_json::json!({"status": "OPEN_REGISTRATION"})));
    }

    #[test]
    fn next_step_after_submit() {
        // The bug: after the last level the game reports one past the end.
        assert_eq!(next_step(5, Some(5), Some(6)).unwrap(), "That was the last level (5 of 5): the game is complete.");
        assert!(next_step(4, Some(5), Some(5)).unwrap().starts_with("Level 5 of 5 is unlocked"));
        assert!(next_step(4, None, Some(5)).unwrap().contains("couldn't tell"));
        assert_eq!(next_step(4, Some(5), Some(4)), None);
    }

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
    fn parses_portal_timestamps() {
        assert_eq!(parse_utc_ms("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_utc_ms("2026-10-19T09:16:22.802336Z"), Some(1_792_401_382_802));
        assert_eq!(parse_utc_ms("2024-02-29T23:59:59.5Z"), Some(1_709_251_199_500));
        assert_eq!(parse_utc_ms("2026-10-19T09:16:22"), None);
        assert_eq!(format_ms(Some(14_400_000)), "4:00:00");
    }

    #[test]
    fn xsrf_from_cookie_header() {
        let agent = agent();
        let portal = Portal { agent: &agent, cookie: "SESSION=abc; XSRF-TOKEN=x-123; other=1" };
        assert_eq!(portal.xsrf(), "x-123");
    }
}

//! Read a site's cookies from Firefox, so logging in to codingcontest.org in the browser is enough.
//!
//! Firefox keeps persistent cookies in `<profile>/cookies.sqlite`. Cookies that only last for the
//! browser session are not written there; they live in the session store
//! (`sessionstore-backups/recovery.jsonlz4` while Firefox runs, `sessionstore.jsonlz4` after it
//! closes). We read both, the session store winning on conflicts. Profiles are tried most recently
//! used first. Files are copied before opening because Firefox keeps the database locked.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::Value;

/// Firefox profiles that have a cookie database, most recently used first.
fn profiles() -> Vec<PathBuf> {
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    let mut roots = Vec::new();
    if let Some(home) = env("HOME") {
        roots.push(home.join(".mozilla/firefox"));
        roots.push(home.join(".config/mozilla/firefox"));
        roots.push(home.join("snap/firefox/common/.mozilla/firefox"));
        roots.push(home.join(".var/app/org.mozilla.firefox/.mozilla/firefox"));
        roots.push(home.join("Library/Application Support/Firefox/Profiles"));
    }
    if let Some(appdata) = env("APPDATA") {
        roots.push(appdata.join("Mozilla/Firefox/Profiles"));
    }
    let mut found: Vec<(SystemTime, PathBuf)> = roots
        .iter()
        .flat_map(|root| fs::read_dir(root).into_iter().flatten())
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter_map(|dir| {
            let used = ["cookies.sqlite", "cookies.sqlite-wal", "sessionstore-backups/recovery.jsonlz4"]
                .iter()
                .filter_map(|f| fs::metadata(dir.join(f)).and_then(|m| m.modified()).ok())
                .max()?;
            Some((used, dir))
        })
        .collect();
    found.sort_by_key(|(used, _)| std::cmp::Reverse(*used));
    found.into_iter().map(|(_, dir)| dir).collect()
}

/// `name=value; ...` for `domain` (exact host or `.domain`, not subdomains) from the most recently
/// used Firefox profile that has the `required` cookie. Returns the header and the profile's name.
pub fn cookie_header(domain: &str, required: &str) -> Result<(String, String), String> {
    let profiles = profiles();
    if profiles.is_empty() {
        return Err("no Firefox profile found".into());
    }
    for profile in &profiles {
        let mut cookies = BTreeMap::new();
        if let Ok(from_db) = read_sqlite(&profile.join("cookies.sqlite"), domain) {
            cookies.extend(from_db);
        }
        for store in ["sessionstore-backups/recovery.jsonlz4", "sessionstore.jsonlz4"] {
            if let Ok(from_session) = read_session_store(&profile.join(store), domain) {
                cookies.extend(from_session);
                break;
            }
        }
        if cookies.contains_key(required) {
            let header = cookies.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("; ");
            let name = profile.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            return Ok((header, name));
        }
    }
    Err(format!("no {required} cookie for {domain} in {} Firefox profile(s); log in there first", profiles.len()))
}

fn host_matches(host: &str, domain: &str) -> bool {
    host == domain || host.strip_prefix('.') == Some(domain)
}

/// Cookies for `domain` from a (copied) `cookies.sqlite`, oldest first so newer ones win.
fn read_sqlite(db: &Path, domain: &str) -> Result<Vec<(String, String)>, String> {
    let tmp = std::env::temp_dir().join(format!("ccc-cookies-{}", std::process::id()));
    fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let result = (|| {
        let copy = tmp.join("cookies.sqlite");
        fs::copy(db, &copy).map_err(|e| e.to_string())?;
        let wal = db.with_file_name("cookies.sqlite-wal");
        if wal.exists() {
            fs::copy(&wal, tmp.join("cookies.sqlite-wal")).map_err(|e| e.to_string())?;
        }
        let conn = rusqlite::Connection::open(&copy).map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT host, name, value FROM moz_cookies ORDER BY creationTime")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))
            .map_err(|e| e.to_string())?;
        Ok(rows
            .filter_map(Result::ok)
            .filter(|(host, _, _)| host_matches(host, domain))
            .map(|(_, name, value)| (name, value))
            .collect())
    })();
    let _ = fs::remove_dir_all(&tmp);
    result
}

/// Cookies for `domain` from a Firefox session store (`mozLz40\0` + u32 size + LZ4 block of JSON).
fn read_session_store(path: &Path, domain: &str) -> Result<Vec<(String, String)>, String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let json = decode_mozlz4(&bytes)?;
    let session: Value = serde_json::from_slice(&json).map_err(|e| e.to_string())?;
    Ok(session["cookies"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| c["host"].as_str().is_some_and(|h| host_matches(h, domain)))
        .filter_map(|c| Some((c["name"].as_str()?.to_owned(), c["value"].as_str()?.to_owned())))
        .collect())
}

fn decode_mozlz4(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let body = bytes.strip_prefix(b"mozLz40\0").ok_or("not a mozLz4 file")?;
    let size = u32::from_le_bytes(body.get(..4).ok_or("truncated")?.try_into().unwrap()) as usize;
    lz4_flex::block::decompress(&body[4..], size).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ccc-test-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reads_matching_cookies_from_sqlite() {
        let dir = temp_dir("sqlite");
        let db = dir.join("cookies.sqlite");
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch(
            "CREATE TABLE moz_cookies (host TEXT, name TEXT, value TEXT, creationTime INTEGER);
             INSERT INTO moz_cookies VALUES ('.codingcontest.org', 'XSRF-TOKEN', 'old', 1);
             INSERT INTO moz_cookies VALUES ('codingcontest.org', 'XSRF-TOKEN', 'new', 2);
             INSERT INTO moz_cookies VALUES ('spaceship.codingcontest.org', 'other', 'x', 3);
             INSERT INTO moz_cookies VALUES ('evil-codingcontest.org', 'SESSION', 'no', 4);",
        )
        .unwrap();
        drop(conn);
        let got = read_sqlite(&db, "codingcontest.org").unwrap();
        assert_eq!(got, [("XSRF-TOKEN".into(), "old".into()), ("XSRF-TOKEN".into(), "new".into())]);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn reads_session_cookies_from_mozlz4() {
        let dir = temp_dir("session");
        let json = br#"{"cookies":[{"host":"codingcontest.org","name":"SESSION","value":"abc"},
                                  {"host":"example.org","name":"SESSION","value":"no"}]}"#;
        let mut file = b"mozLz40\0".to_vec();
        file.extend_from_slice(&(json.len() as u32).to_le_bytes());
        file.extend_from_slice(&lz4_flex::block::compress(json));
        let path = dir.join("recovery.jsonlz4");
        fs::write(&path, file).unwrap();
        assert_eq!(read_session_store(&path, "codingcontest.org").unwrap(), [("SESSION".into(), "abc".into())]);
        fs::remove_dir_all(dir).unwrap();
    }
}

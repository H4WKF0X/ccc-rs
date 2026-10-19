//! Client for the codingcontest.org platform (replaces CatCoder from 2026). NOT WRITTEN YET.
//!
//! What we know from the portal's public JavaScript (details in docs/PLATFORM.md):
//!
//! 1. The portal `https://codingcontest.org` authenticates with a session cookie.
//! 2. `POST /api/game-token {"contestSlug": ...}` returns `{"token": ...}` for the running contest.
//! 3. Each game is its own app (`GET /api/games` gives its `baseUrl`, e.g.
//!    `https://spaceship.codingcontest.org/`). The portal hands the token to it with
//!    `postMessage({type: "ccc:auth-token", token, contestSlug, ...})`.
//! 4. Files are probably served from `https://files.codingcontest.org` (it is in the game's CSP).
//!
//! What we still need, from a HAR capture of a training run: the game's download and upload
//! endpoints, how the token is sent (header? cookie?), what the upload response looks like, and
//! where the "0000" code comes in.

use std::path::PathBuf;

use crate::config::Config;

pub fn submit(_cfg: &Config, _level: u32, _outputs: &[PathBuf], _yes: bool) -> Result<bool, String> {
    Err("the codingcontest.org client is not written yet (the game API is unknown until we \
         capture a HAR; see docs/PLATFORM.md). Set platform = \"manual\" in ccc.toml for now."
        .into())
}

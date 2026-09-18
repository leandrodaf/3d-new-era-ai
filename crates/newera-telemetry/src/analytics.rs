//! How many people open the app, and on what.
//!
//! One event per run — `app_open`, with the version, the operating system and
//! the mode the app was started in — sent to the same Google Analytics
//! property the site reports to, through the Measurement Protocol. It rides
//! the switch the rest of this crate rides: off means nothing leaves.
//!
//! What is sent: an id drawn at random the first time and kept in the config
//! directory, the app version, the OS family, the mode and the interface
//! language. What is not: the project, its path, the plan, the MCP token, the
//! machine's name, the user's name. The id names an installation so two runs
//! can be told apart from two machines; it is not a person and it is thrown
//! away with the settings directory.
//!
//! The API secret is baked in at build time from `NEWERA_GA_API_SECRET`, the
//! way the Sentry DSN is. A build without it sends nothing, so a clone of the
//! repository reports to nobody.

use std::time::Duration;

/// The property the site and the app share.
const MEASUREMENT_ID: &str = "G-PLY5GQC6EP";

/// Google's collector. `debug` is a different host that answers with what it
/// thinks of the payload; useful when adding an event, not in a release.
const COLLECT: &str = "https://www.google-analytics.com/mp/collect";

/// The secret this build sends with, if it has one.
fn api_secret() -> Option<String> {
    let given = |s: &String| !s.trim().is_empty();
    std::env::var("NEWERA_GA_API_SECRET")
        .ok()
        .filter(given)
        .or_else(|| option_env!("NEWERA_GA_API_SECRET").map(str::to_owned))
        .filter(given)
}

/// The id of this installation, drawn once and kept beside the settings. Not
/// a person: reinstalling draws another one.
fn client_id() -> std::io::Result<String> {
    client_id_in(&super::config_dir())
}

/// The same, in a named directory — which is what the test uses, so it never
/// touches the settings of the machine it runs on.
fn client_id_in(dir: &std::path::Path) -> std::io::Result<String> {
    let path = dir.join("install-id");
    if let Ok(id) = std::fs::read_to_string(&path) {
        let id = id.trim().to_owned();
        if !id.is_empty() {
            return Ok(id);
        }
    }
    // Enough randomness to not collide, from what the standard library has:
    // the clock and the address of a fresh allocation.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let here = std::ptr::from_ref::<String>(&String::new()) as usize;
    let id = format!("{:x}{here:x}", now.as_nanos());
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, &id)?;
    Ok(id)
}

/// Reports that the app was opened, in the background: a run never waits on
/// Google, and a refusal from the network is not worth a word on screen.
///
/// `mode` is how it was started — `gui`, `serve`, `mcp` — so a headless
/// server is not counted as somebody sitting in front of a window.
pub fn opened(mode: &str) {
    if !super::enabled() {
        return;
    }
    let Some(secret) = api_secret() else {
        return;
    };
    let Ok(client) = client_id() else {
        return;
    };
    let body = serde_json::json!({
        "client_id": client,
        "non_personalized_ads": true,
        "events": [{
            "name": "app_open",
            "params": {
                "engagement_time_msec": "1",
                "session_id": client,
                "app_version": env!("CARGO_PKG_VERSION"),
                "operating_system": std::env::consts::OS,
                "architecture": std::env::consts::ARCH,
                "mode": mode,
            }
        }]
    });
    let url = format!("{COLLECT}?measurement_id={MEASUREMENT_ID}&api_secret={secret}");
    std::thread::spawn(move || {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(4)))
            .build()
            .into();
        // Whatever comes back is dropped: the call is a ping, and a failed
        // ping is not news.
        let _ = agent.post(&url).send_json(&body);
    });
}

#[cfg(test)]
mod tests {
    /// Without a secret nothing is sent, and a clone of the repository has no
    /// secret: this is what keeps someone else's build from reporting to us.
    #[test]
    fn a_build_without_a_secret_sends_nothing() {
        if std::env::var("NEWERA_GA_API_SECRET").is_ok()
            || option_env!("NEWERA_GA_API_SECRET").is_some_and(|s| !s.trim().is_empty())
        {
            return; // built with one: nothing to prove here
        }
        assert!(super::api_secret().is_none());
        // Nothing to send to, so this returns without a thread and without a
        // request.
        super::opened("test");
    }

    /// The id is drawn once and then answers the same for this installation.
    #[test]
    fn the_installation_id_is_kept() {
        let dir = std::env::temp_dir().join(format!("newera-ga-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let first = super::client_id_in(&dir).expect("an id");
        let again = super::client_id_in(&dir).expect("the same id");
        assert_eq!(first, again);
        assert!(!first.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

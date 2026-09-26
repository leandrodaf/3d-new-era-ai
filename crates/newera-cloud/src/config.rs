//! What the service needs to know about where it runs, from the environment.

use anyhow::Context as _;

/// How sign-in links reach people.
#[derive(Clone)]
pub enum Mail {
    /// Through Resend's HTTP API.
    Resend { api_key: String, from: String },
    /// Into the log — for a machine that is only being tried out. Never in
    /// production: whoever reads the log could sign in as anyone.
    Log,
    /// Not set up yet: sign-in by email is off, and everything else — the
    /// relay above all — keeps working.
    Off,
}

/// Google as a way to sign in, when configured.
#[derive(Clone)]
pub struct Google {
    pub client_id: String,
    pub client_secret: String,
}

// Secrets stay out of logs and panics.
impl std::fmt::Debug for Mail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resend { from, .. } => {
                write!(f, "Resend {{ from: {from:?}, api_key: <hidden> }}")
            }
            Self::Log => f.write_str("Log"),
            Self::Off => f.write_str("Off"),
        }
    }
}

impl std::fmt::Debug for Google {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Google {{ client_id: {:?}, client_secret: <hidden> }}",
            self.client_id
        )
    }
}

/// Buy Me a Coffee, where the paid plan is supported: it sells the
/// membership on a page of its own and pays out through the Stripe account
/// connected to it. This service only points people at that page and listens
/// to what it says afterwards.
#[derive(Clone)]
pub struct BuyMeACoffee {
    /// The page's name, the part after `buymeacoffee.com/`.
    pub page: String,
    /// The webhook's signing secret: only deliveries signed with it change a
    /// plan.
    pub webhook_secret: String,
    /// The membership levels that pay for the plan, by id. Empty means every
    /// level does, which is right for a page that sells only this.
    pub levels: Vec<i64>,
    /// Whether a test event from the dashboard (`live_mode: false`) may
    /// change a plan. Off in production, where only real money should.
    pub test_events: bool,
}

impl BuyMeACoffee {
    /// Reads `BUYMEACOFFEE_*`: both the page and the webhook secret, or
    /// neither (the paid plan off).
    fn from_env(var: &dyn Fn(&str) -> Option<String>) -> anyhow::Result<Option<Self>> {
        let required = ["BUYMEACOFFEE_PAGE", "BUYMEACOFFEE_WEBHOOK_SECRET"];
        let values: Vec<Option<String>> = required.iter().map(|name| var(name)).collect();
        if values.iter().all(Option::is_none) {
            tracing::warn!("no BUYMEACOFFEE_* settings: the paid plan is off");
            return Ok(None);
        }
        let missing: Vec<&str> = required
            .iter()
            .zip(&values)
            .filter(|(_, v)| v.is_none())
            .map(|(name, _)| *name)
            .collect();
        anyhow::ensure!(missing.is_empty(), "missing {}", missing.join(", "));
        let mut values = values.into_iter().map(Option::unwrap_or_default);
        let mut next = || values.next().unwrap_or_default();
        // A whole URL where a name was asked for is the easy mistake to make.
        let page = next()
            .trim()
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_start_matches("www.")
            .trim_start_matches("buymeacoffee.com/")
            .trim_matches('/')
            .to_owned();
        anyhow::ensure!(!page.is_empty(), "BUYMEACOFFEE_PAGE is empty");
        let webhook_secret = next();
        let levels = var("BUYMEACOFFEE_LEVELS")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(|l| l.parse::<i64>().context("BUYMEACOFFEE_LEVELS"))
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(Some(Self {
            page,
            webhook_secret,
            levels,
            test_events: matches!(
                var("BUYMEACOFFEE_TEST_EVENTS").as_deref(),
                Some("1" | "true")
            ),
        }))
    }
}

impl std::fmt::Debug for BuyMeACoffee {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BuyMeACoffee")
            .field("page", &self.page)
            .field("webhook_secret", &"<hidden>")
            .field("levels", &self.levels)
            .field("test_events", &self.test_events)
            .finish()
    }
}

#[derive(Clone)]
pub struct Config {
    /// Where this service is reached, without a trailing slash, e.g.
    /// `https://mcp.3dneweraai.com`. The issuer, and the base of every URL
    /// it hands out.
    pub public_url: String,
    pub database_url: String,
    /// Pages allowed to call the account endpoints with the person's cookie:
    /// the editor at 3dneweraai.com.
    pub site_origins: Vec<String>,
    pub mail: Mail,
    pub google: Option<Google>,
    /// The browser editor, where "open in the editor" goes.
    pub editor_url: String,
    /// The paid plan, supported through Buy Me a Coffee; `None` leaves every
    /// account on the free plan and the account page without a way to pay.
    pub buymeacoffee: Option<BuyMeACoffee>,
    /// The port to listen on, on every interface of the container; the
    /// Cloudflare tunnel in front is what reaches it.
    pub port: u16,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("public_url", &self.public_url)
            .field("database_url", &"<hidden>")
            .field("site_origins", &self.site_origins)
            .field("mail", &self.mail)
            .field("google", &self.google)
            .field("editor_url", &self.editor_url)
            .field("buymeacoffee", &self.buymeacoffee)
            .field("port", &self.port)
            .finish()
    }
}

impl Config {
    /// Reads the configuration.
    ///
    /// # Errors
    ///
    /// When a required variable is missing or malformed.
    pub fn from_env() -> anyhow::Result<Self> {
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.trim().is_empty());
        let public_url = var("NEWERA_PUBLIC_URL")
            .unwrap_or_else(|| "https://mcp.3dneweraai.com".to_owned())
            .trim_end_matches('/')
            .to_owned();
        let database_url = var("DATABASE_URL").context("DATABASE_URL is required")?;
        let site_origins = var("NEWERA_SITE_ORIGINS")
            .unwrap_or_else(|| "https://3dneweraai.com,https://www.3dneweraai.com".to_owned())
            .split(',')
            .map(|o| o.trim().trim_end_matches('/').to_owned())
            .filter(|o| !o.is_empty())
            .collect();
        let mail = match (var("RESEND_API_KEY"), var("NEWERA_MAIL")) {
            (Some(api_key), _) => Mail::Resend {
                api_key,
                from: var("NEWERA_MAIL_FROM")
                    .unwrap_or_else(|| "3D New Era AI <entrar@3dneweraai.com>".to_owned()),
            },
            (None, Some(mode)) if mode == "log" => Mail::Log,
            _ => {
                tracing::warn!(
                    "no RESEND_API_KEY: sign-in by email is off (NEWERA_MAIL=log prints links, for trying it out)"
                );
                Mail::Off
            }
        };
        let google = match (var("GOOGLE_CLIENT_ID"), var("GOOGLE_CLIENT_SECRET")) {
            (Some(client_id), Some(client_secret)) => Some(Google {
                client_id,
                client_secret,
            }),
            _ => None,
        };
        let editor_url =
            var("NEWERA_EDITOR_URL").unwrap_or_else(|| "https://3dneweraai.com/app/".to_owned());
        let buymeacoffee = BuyMeACoffee::from_env(&var)?;
        let port = var("PORT")
            .map(|p| p.parse().context("PORT"))
            .transpose()?
            .unwrap_or(7979);
        Ok(Self {
            public_url,
            database_url,
            site_origins,
            mail,
            google,
            editor_url,
            buymeacoffee,
            port,
        })
    }

    /// Cookies go over HTTPS only, unless this runs on a plain-HTTP address
    /// for local testing.
    pub fn secure_cookies(&self) -> bool {
        self.public_url.starts_with("https://")
    }
}

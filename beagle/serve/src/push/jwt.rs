//! The APNs provider token: an ES256 JWT signed with the `.p8` key from the
//! Apple developer account, reused for [`REUSE_FOR`].

use std::time::Duration;

use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine as _;
use ring::rand::SystemRandom;
use ring::signature::{EcdsaKeyPair, KeyPair as _, ECDSA_P256_SHA256_FIXED_SIGNING};
use serde_json::json;

/// How long one token is used. APNs rejects tokens older than an hour and
/// throttles ones refreshed more often than every 20 minutes.
pub const REUSE_FOR: Duration = Duration::from_mins(50);

/// The `.p8` key, ready to sign.
#[derive(Debug)]
pub struct Signer {
    key: EcdsaKeyPair,
    team_id: String,
    key_id: String,
    rng: SystemRandom,
    cached: Option<(u64, String)>,
}

impl Signer {
    /// A signer from the `.p8` file's text (PEM, PKCS#8, P-256).
    ///
    /// # Errors
    /// What is wrong with the key, for a person.
    pub fn new(pem: &str, team_id: &str, key_id: &str) -> Result<Self, String> {
        let der = pem_to_der(pem)?;
        let rng = SystemRandom::new();
        let key = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &der, &rng)
            .map_err(|e| format!("not an APNs key (PKCS#8, P-256): {e}"))?;
        Ok(Self {
            key,
            team_id: team_id.to_owned(),
            key_id: key_id.to_owned(),
            rng,
            cached: None,
        })
    }

    /// The token to send at unix time `now`: the cached one while it is
    /// younger than [`REUSE_FOR`], else a new one.
    ///
    /// # Errors
    /// The signing error (the system RNG failed).
    pub fn token(&mut self, now: u64) -> Result<String, String> {
        if let Some((issued, token)) = &self.cached {
            if now.saturating_sub(*issued) < REUSE_FOR.as_secs() {
                return Ok(token.clone());
            }
        }
        let header = json!({ "alg": "ES256", "kid": self.key_id });
        let claims = json!({ "iss": self.team_id, "iat": now });
        let signing_input = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(header.to_string()),
            URL_SAFE_NO_PAD.encode(claims.to_string())
        );
        let signature = self
            .key
            .sign(&self.rng, signing_input.as_bytes())
            .map_err(|e| format!("signing the APNs token failed: {e}"))?;
        let token = format!(
            "{signing_input}.{}",
            URL_SAFE_NO_PAD.encode(signature.as_ref())
        );
        self.cached = Some((now, token.clone()));
        Ok(token)
    }

    /// Drops the cached token, so the next one is new (after APNs says
    /// `ExpiredProviderToken`).
    pub fn expire(&mut self) {
        self.cached = None;
    }

    /// The key's public half (uncompressed point), for tests to verify.
    #[must_use]
    pub fn public_key(&self) -> &[u8] {
        self.key.public_key().as_ref()
    }
}

/// The DER bytes inside a `-----BEGIN PRIVATE KEY-----` PEM block.
fn pem_to_der(pem: &str) -> Result<Vec<u8>, String> {
    let body: String = pem
        .lines()
        .map(str::trim)
        .skip_while(|l| *l != "-----BEGIN PRIVATE KEY-----")
        .skip(1)
        .take_while(|l| *l != "-----END PRIVATE KEY-----")
        .collect();
    if body.is_empty() {
        return Err("no `-----BEGIN PRIVATE KEY-----` block: is this the .p8 file?".into());
    }
    STANDARD
        .decode(body)
        .map_err(|e| format!("the key's base64 is broken: {e}"))
}

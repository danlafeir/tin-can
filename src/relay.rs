use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

const RELAY_BASE: &str = "https://lafeir.com/api";

#[derive(Serialize)]
struct UploadOfferBody {
    offer: String,
}

#[derive(Serialize)]
struct PutKnotTieBody {
    #[serde(rename = "knot-tie")]
    knot_tie: String,
}

#[derive(Deserialize)]
struct GetOfferResponse {
    offer: String,
}

#[derive(Deserialize)]
struct GetKnotTieResponse {
    #[serde(rename = "knot-tie")]
    knot_tie: String,
}

pub struct RelayClient {
    client: reqwest::blocking::Client,
    base: String,
}

impl RelayClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("build http client"),
            base: RELAY_BASE.to_string(),
        }
    }

    /// Upload an offer at a client-chosen code (derived from shared secret).
    pub fn upload_offer(&self, code: &str, offer_b64: &str) -> Result<()> {
        self.client
            .put(format!("{}/string?code={}", self.base, code))
            .json(&UploadOfferBody {
                offer: offer_b64.to_string(),
            })
            .send()
            .context("failed to reach relay")?
            .error_for_status()
            .context("relay rejected offer")?;
        Ok(())
    }

    /// Fetch the offer for a code. Errors on 404.
    pub fn get_offer(&self, code: &str) -> Result<String> {
        let resp = self
            .client
            .get(format!("{}/string?code={}", self.base, code))
            .send()
            .context("failed to reach relay")?;

        if resp.status().as_u16() == 404 {
            bail!("no session found — check the secret and try again");
        }

        resp.error_for_status()
            .context("relay error fetching offer")?
            .json::<GetOfferResponse>()
            .context("invalid offer from relay")
            .map(|r| r.offer)
    }

    /// Like get_offer, but returns None on 404.
    /// Used by `talk` to auto-detect offerer vs answerer role.
    pub fn try_get_offer(&self, code: &str) -> Result<Option<String>> {
        let resp = self
            .client
            .get(format!("{}/string?code={}", self.base, code))
            .send()
            .context("failed to reach relay")?;

        if resp.status().as_u16() == 404 {
            return Ok(None);
        }

        resp.error_for_status()
            .context("relay error fetching offer")?
            .json::<GetOfferResponse>()
            .context("invalid offer from relay")
            .map(|r| Some(r.offer))
    }

    pub fn put_knot_tie(&self, code: &str, knot_tie_b64: &str) -> Result<()> {
        self.client
            .put(format!("{}/knot-tie?code={}", self.base, code))
            .json(&PutKnotTieBody {
                knot_tie: knot_tie_b64.to_string(),
            })
            .send()
            .context("failed to reach relay")?
            .error_for_status()
            .context("relay rejected knot-tie")?;
        Ok(())
    }

    /// Returns Some(knot_tie_b64) when the peer has responded, None if still waiting.
    pub fn poll_knot_tie(&self, code: &str) -> Result<Option<String>> {
        let resp = self
            .client
            .get(format!("{}/knot-tie?code={}", self.base, code))
            .send()
            .context("failed to reach relay")?;

        if resp.status().as_u16() == 204 {
            return Ok(None);
        }

        resp.error_for_status()
            .context("relay error polling for knot-tie")?
            .json::<GetKnotTieResponse>()
            .context("invalid knot-tie response from relay")
            .map(|r| Some(r.knot_tie))
    }
}

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const RELAY_BASE: &str = "https://lafeir.com/api";

#[derive(Serialize)]
struct UploadOfferBody {
    offer: String,
}

#[derive(Serialize)]
struct PutAnswerBody {
    answer: String,
}

#[derive(Deserialize)]
struct GetOfferResponse {
    offer: String,
}

#[derive(Deserialize)]
struct GetAnswerResponse {
    answer: String,
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
            .put(format!("{}/room?code={}", self.base, code))
            .json(&UploadOfferBody {
                offer: offer_b64.to_string(),
            })
            .send()
            .context("failed to reach relay")?
            .error_for_status()
            .context("relay rejected offer")?;
        Ok(())
    }

    /// Fetch the offer for a code; returns None on 404.
    /// Used by commands to auto-detect offerer vs answerer role.
    pub fn try_get_offer(&self, code: &str) -> Result<Option<String>> {
        let resp = self
            .client
            .get(format!("{}/room?code={}", self.base, code))
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

    pub fn put_answer(&self, code: &str, answer_b64: &str) -> Result<()> {
        self.client
            .put(format!("{}/answer?code={}", self.base, code))
            .json(&PutAnswerBody {
                answer: answer_b64.to_string(),
            })
            .send()
            .context("failed to reach relay")?
            .error_for_status()
            .context("relay rejected answer")?;
        Ok(())
    }

    /// Returns Some(answer_b64) when the peer has responded, None if still waiting.
    pub fn poll_answer(&self, code: &str) -> Result<Option<String>> {
        let resp = self
            .client
            .get(format!("{}/answer?code={}", self.base, code))
            .send()
            .context("failed to reach relay")?;

        if resp.status().as_u16() == 204 {
            return Ok(None);
        }

        resp.error_for_status()
            .context("relay error polling for answer")?
            .json::<GetAnswerResponse>()
            .context("invalid answer response from relay")
            .map(|r| Some(r.answer))
    }
}

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

const RELAY_BASE: &str = "https://lafeir.com/api";

#[derive(Serialize)]
struct CreateRoomBody {
    offer: String,
}

#[derive(Deserialize)]
struct CreateRoomResponse {
    code: String,
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
    answer: Option<String>,
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

    pub fn create_room(&self, offer_b64: &str) -> Result<String> {
        let resp = self
            .client
            .post(format!("{}/room", self.base))
            .json(&CreateRoomBody {
                offer: offer_b64.to_string(),
            })
            .send()
            .context("failed to reach relay")?
            .error_for_status()
            .context("relay rejected room creation")?
            .json::<CreateRoomResponse>()
            .context("invalid room response from relay")?;
        Ok(resp.code)
    }

    pub fn get_offer(&self, code: &str) -> Result<String> {
        let resp = self
            .client
            .get(format!("{}/room?code={}", self.base, code))
            .send()
            .context("failed to reach relay")?;

        if resp.status().as_u16() == 404 {
            bail!("room '{}' not found — check the code and try again", code);
        }

        resp.error_for_status()
            .context("relay error fetching offer")?
            .json::<GetOfferResponse>()
            .context("invalid offer from relay")
            .map(|r| r.offer)
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
            .map(|r| r.answer)
    }
}

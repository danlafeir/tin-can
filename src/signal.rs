use anyhow::{Context, Result};
use base64::Engine;
use base64::engine::general_purpose::{STANDARD_NO_PAD, URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};
use str0m::change::{SdpAnswer, SdpOffer};

const CAN_BASE_URL: &str = "https://daniellafeir.com/can/";

// ── Room code derivation (relay mode) ─────────────────────────────────────────

/// Derive a relay room code from a shared secret.
/// Uses the first 12 hex chars of SHA-256(secret) — not reversible to the secret.
pub fn derive_room_code(secret: &str) -> String {
    let hash = Sha256::digest(secret.as_bytes());
    format!("{:x}", hash)[..12].to_string()
}

// ── Base64 encoding (relay transport) ────────────────────────────────────────

pub fn encode_offer(offer: &SdpOffer) -> Result<String> {
    let json = serde_json::to_string(offer).context("serialize offer")?;
    Ok(STANDARD_NO_PAD.encode(json.as_bytes()))
}

pub fn decode_offer(blob: &str) -> Result<SdpOffer> {
    let json_bytes = STANDARD_NO_PAD
        .decode(blob.trim())
        .context("base64 decode offer")?;
    serde_json::from_slice(&json_bytes).context("parse offer JSON")
}

pub fn encode_answer(answer: &SdpAnswer) -> Result<String> {
    let json = serde_json::to_string(answer).context("serialize answer")?;
    Ok(STANDARD_NO_PAD.encode(json.as_bytes()))
}

pub fn decode_answer(blob: &str) -> Result<SdpAnswer> {
    let json_bytes = STANDARD_NO_PAD
        .decode(blob.trim())
        .context("base64 decode answer")?;
    serde_json::from_slice(&json_bytes).context("parse answer JSON")
}

// ── URL encoding (daniellafeir.com/can/ static-link mode) ────────────────────

pub fn offer_to_url(offer: &SdpOffer) -> Result<String> {
    let json = serde_json::to_string(offer).context("serialize offer")?;
    let b64 = URL_SAFE_NO_PAD.encode(json.as_bytes());
    Ok(format!("{}#o={}", CAN_BASE_URL, b64))
}

pub fn offer_from_url(url: &str) -> Result<SdpOffer> {
    let b64 = extract_fragment(url, "o")?;
    let json = URL_SAFE_NO_PAD.decode(b64).context("base64 decode offer URL")?;
    serde_json::from_slice(&json).context("parse offer JSON from URL")
}

pub fn answer_to_url(answer: &SdpAnswer) -> Result<String> {
    let json = serde_json::to_string(answer).context("serialize answer")?;
    let b64 = URL_SAFE_NO_PAD.encode(json.as_bytes());
    Ok(format!("{}#a={}", CAN_BASE_URL, b64))
}

/// Accept either a `can/` answer URL or a raw base64 blob.
pub fn answer_from_input(input: &str) -> Result<SdpAnswer> {
    let input = input.trim();
    if input.starts_with("http://") || input.starts_with("https://") {
        let b64 = extract_fragment(input, "a")?;
        let json = URL_SAFE_NO_PAD.decode(b64).context("base64 decode answer URL")?;
        serde_json::from_slice(&json).context("parse answer JSON from URL")
    } else {
        decode_answer(input)
    }
}

fn extract_fragment<'a>(url: &'a str, key: &str) -> Result<&'a str> {
    let hash = url
        .split_once('#')
        .map(|(_, h)| h)
        .context("URL has no # fragment")?;
    let prefix = format!("{}=", key);
    hash.strip_prefix(prefix.as_str())
        .with_context(|| format!("fragment does not start with '{}='", key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relay_round_trip() {
        let sdp_text = "v=0\r\no=str0m-0 1234 2 IN IP4 0.0.0.0\r\ns=-\r\nt=0 0\r\n";
        let offer = SdpOffer::from_sdp_string(sdp_text).expect("parse sdp");
        let encoded = encode_offer(&offer).expect("encode");
        let decoded = decode_offer(&encoded).expect("decode");
        assert_eq!(offer.to_sdp_string(), decoded.to_sdp_string());
    }

    #[test]
    fn offer_url_round_trip() {
        let sdp_text = "v=0\r\no=str0m-0 1234 2 IN IP4 0.0.0.0\r\ns=-\r\nt=0 0\r\n";
        let offer = SdpOffer::from_sdp_string(sdp_text).expect("parse sdp");
        let url = offer_to_url(&offer).expect("encode url");
        assert!(url.contains("#o="));
        let decoded = offer_from_url(&url).expect("decode url");
        assert_eq!(offer.to_sdp_string(), decoded.to_sdp_string());
    }

    #[test]
    fn answer_from_input_accepts_url_and_raw() {
        let sdp_text = "v=0\r\no=str0m-0 1234 2 IN IP4 0.0.0.0\r\ns=-\r\nt=0 0\r\n";
        let answer = SdpAnswer::from_sdp_string(sdp_text).expect("parse sdp");

        let url = answer_to_url(&answer).expect("encode url");
        let from_url = answer_from_input(&url).expect("decode from url");
        assert_eq!(answer.to_sdp_string(), from_url.to_sdp_string());

        let raw = encode_answer(&answer).expect("encode raw");
        let from_raw = answer_from_input(&raw).expect("decode from raw");
        assert_eq!(answer.to_sdp_string(), from_raw.to_sdp_string());
    }

    #[test]
    fn derive_room_code_deterministic() {
        let code = derive_room_code("hello world");
        assert_eq!(code.len(), 12);
        assert!(code.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(code, derive_room_code("hello world"));
        assert_ne!(code, derive_room_code("hello world!"));
    }
}

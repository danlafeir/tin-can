use anyhow::{Context, Result};
use base64::Engine;
use base64::engine::general_purpose::STANDARD_NO_PAD;
use str0m::change::{SdpAnswer, SdpOffer};

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offer_round_trip() {
        // Use a real SDP string that str0m can parse
        let sdp_text = "v=0\r\no=str0m-0 1234 2 IN IP4 0.0.0.0\r\ns=-\r\nt=0 0\r\n";
        let offer = SdpOffer::from_sdp_string(sdp_text).expect("parse sdp");
        let encoded = encode_offer(&offer).expect("encode");
        let decoded = decode_offer(&encoded).expect("decode");
        assert_eq!(offer.to_sdp_string(), decoded.to_sdp_string());
    }

    #[test]
    fn answer_round_trip() {
        let sdp_text = "v=0\r\no=str0m-0 1234 2 IN IP4 0.0.0.0\r\ns=-\r\nt=0 0\r\n";
        let answer = SdpAnswer::from_sdp_string(sdp_text).expect("parse sdp");
        let encoded = encode_answer(&answer).expect("encode");
        let decoded = decode_answer(&encoded).expect("decode");
        assert_eq!(answer.to_sdp_string(), decoded.to_sdp_string());
    }
}

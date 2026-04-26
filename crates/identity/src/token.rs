use serde::{Deserialize, Serialize};

use crate::workload::WorkloadId;

/// An opaque identity token issued to a workload.
///
/// In a production system this would be a signed JWT or similar credential.
/// For now it carries structured claims that can be verified locally.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityToken {
    raw: String,
    claims: TokenClaims,
}

/// The claims embedded in an identity token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenClaims {
    pub subject: WorkloadId,
    pub capabilities: Vec<String>,
    pub issued_at_unix_ms: u128,
    pub expires_at_unix_ms: u128,
}

impl IdentityToken {
    /// Issue a new token for the given claims.
    ///
    /// The raw representation is a base64-encoded JSON payload. A real
    /// implementation would sign this with a key.
    pub fn issue(claims: TokenClaims) -> anyhow::Result<Self> {
        let json = serde_json::to_vec(&claims)?;
        let raw = base64_encode(&json);
        Ok(Self { raw, claims })
    }

    /// Decode a token from its raw string representation.
    pub fn decode(raw: &str) -> anyhow::Result<Self> {
        let bytes = base64_decode(raw)?;
        let claims: TokenClaims = serde_json::from_slice(&bytes)?;
        Ok(Self {
            raw: raw.to_string(),
            claims,
        })
    }

    pub fn raw(&self) -> &str {
        &self.raw
    }

    pub fn claims(&self) -> &TokenClaims {
        &self.claims
    }
}

fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(ALPHABET[((triple >> 18) & 0x3F) as usize] as char);
        out.push(ALPHABET[((triple >> 12) & 0x3F) as usize] as char);

        if chunk.len() > 1 {
            out.push(ALPHABET[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }

        if chunk.len() > 2 {
            out.push(ALPHABET[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn base64_decode(input: &str) -> anyhow::Result<Vec<u8>> {
    fn decode_char(c: u8) -> anyhow::Result<u8> {
        match c {
            b'A'..=b'Z' => Ok(c - b'A'),
            b'a'..=b'z' => Ok(c - b'a' + 26),
            b'0'..=b'9' => Ok(c - b'0' + 52),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => anyhow::bail!("invalid base64 character: {}", c as char),
        }
    }

    let input = input.as_bytes();
    let mut out = Vec::with_capacity(input.len() * 3 / 4);

    for chunk in input.chunks(4) {
        if chunk.len() < 4 {
            anyhow::bail!("invalid base64 length");
        }

        let pad = chunk.iter().filter(|&&b| b == b'=').count();
        let vals: Vec<u8> = chunk
            .iter()
            .filter(|&&b| b != b'=')
            .map(|&b| decode_char(b))
            .collect::<anyhow::Result<Vec<u8>>>()?;

        if vals.len() + pad != 4 {
            anyhow::bail!("invalid base64 padding");
        }

        let v0 = *vals.first().unwrap_or(&0) as u32;
        let v1 = *vals.get(1).unwrap_or(&0) as u32;
        let v2 = *vals.get(2).unwrap_or(&0) as u32;
        let v3 = *vals.get(3).unwrap_or(&0) as u32;
        let triple = (v0 << 18) | (v1 << 12) | (v2 << 6) | v3;

        out.push(((triple >> 16) & 0xFF) as u8);
        if pad < 2 {
            out.push(((triple >> 8) & 0xFF) as u8);
        }
        if pad < 1 {
            out.push((triple & 0xFF) as u8);
        }
    }

    Ok(out)
}

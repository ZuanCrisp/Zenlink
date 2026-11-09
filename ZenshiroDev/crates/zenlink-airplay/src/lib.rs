//! AirPlay 1 screen-mirroring receiver (an Apple TV 3 to iOS), port of
//! `apps/desktop/backend/zenlink/airplay` (protocol after UxPlay / shairplay, GPL-3.0).
//! Serves ZenLink Studio over Wi-Fi and the Hybrid picture (iOS 17-26).
//!
//! Step 1 (this file set): receiver identity, `GET /info`, pair-verify, FairPlay and the
//! session and stream keys. RTSP, mDNS, the mirror stream and audio follow (ROADMAP "M3 plan").

pub mod fairplay;
pub mod identity;
pub mod info;
pub mod pair;

use sha2::{Digest, Sha512};

/// AES-128 in CTR mode with a full 128-bit big-endian counter (PyCryptodome's
/// `MODE_CTR, nonce=b"", initial_value=iv`).
pub type Ctr = ctr::Ctr128BE<aes::Aes128>;

pub fn ctr(key: &[u8; 16], iv: &[u8; 16]) -> Ctr {
    use aes::cipher::KeyIvInit;
    Ctr::new(key.into(), iv.into())
}

/// The first 16 bytes of SHA-512 over `parts`.
pub fn sha512_16(parts: &[&[u8]]) -> [u8; 16] {
    let mut h = Sha512::new();
    for p in parts {
        h.update(p);
    }
    h.finalize()[..16].try_into().unwrap()
}

/// The session's AES key: FairPlay's key, mixed with the pair-verify secret when there is one.
pub fn session_key(fairplay_key: &[u8; 16], ecdh: Option<&[u8; 32]>) -> [u8; 16] {
    match ecdh {
        Some(s) => sha512_16(&[fairplay_key, s]),
        None => *fairplay_key,
    }
}

/// Key and IV of a mirror stream. `id` is iOS's `streamConnectionID`, unsigned (plist parsers
/// read the 8-byte integer as signed: half the sessions had a wrong key).
pub fn stream_key_iv(aeskey: &[u8; 16], id: u64) -> ([u8; 16], [u8; 16]) {
    (
        sha512_16(&[format!("AirPlayStreamKey{id}").as_bytes(), aeskey]),
        sha512_16(&[format!("AirPlayStreamIV{id}").as_bytes(), aeskey]),
    )
}

pub(crate) fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

#[cfg(test)]
pub(crate) mod vectors {
    //! From the Python receiver (scratch script ap_vectors.py, 2026-11-09): server seed
    //! 00..1f, server X25519 secret 07*32, client X25519 09*32, client Ed25519 05*32.
    pub const SERVER_PK: &str = "03a107bff3ce10be1d70dd18e74bc09967e4d6309ba50d5f1ddc8664125531b8";
    pub const CLIENT_X: &str = "57db4b359f23ae5e146e4e2512056704722506348c150c14753d0c933d04d421";
    pub const CLIENT_ED: &str = "6e7a1cdd29b0b78fd13af4c5598feff4ef2a97166e3ca6f2e4fbfccd80505bf1";
    pub const REPLY1: &str = "13be4feaeaf204c7fd3358fc9c00721881d174278128227ec674f37f7fe97b6dbd7006b5fbd3c8778c39f16495e20bb721bbf896df1a3c421d2609e0b9581791dd76f3bbdc72550fc5d07d5e3052ebdc4ac629412ec5b571a1db4ff9842fd45b";
    pub const BODY2: &str = "00000000bd00495d29ae2f78fd10a77dcfb24232642ad2fc024a2b4c27a9b1e72e44e59a3d8a1599ac9755412a71e86b50acf11fde2db2cd3e5cb50ed71c7dcb243b0d0c";
    pub const SECRET: &str = "2ffe325b2a26611bf1911325d4a62b33749b8f350d0e88ba91e316acaae66554";
    pub const FP_KEY: &str = "299464065e68f7ed2125fcdd9a06f983";
    pub const AESKEY: &str = "4436577e966f8640f065b7348e0e32ac";
    pub const STREAM_KEY: &str = "c0b93b58aa905bdef02f3d95aa731285";
    pub const STREAM_IV: &str = "9cc97499162c789ccd790377beb85b2b";
}

#[cfg(test)]
mod tests {
    use super::{hex, vectors as v, *};

    #[test]
    fn session_and_stream_keys_match_python() {
        let fp: [u8; 16] = hex(v::FP_KEY).try_into().unwrap();
        let secret: [u8; 32] = hex(v::SECRET).try_into().unwrap();
        let key = session_key(&fp, Some(&secret));
        assert_eq!(key.to_vec(), hex(v::AESKEY));
        let (k, iv) = stream_key_iv(&key, (1 << 63) + 5);
        assert_eq!(k.to_vec(), hex(v::STREAM_KEY));
        assert_eq!(iv.to_vec(), hex(v::STREAM_IV));
    }
}

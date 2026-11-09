//! Legacy AirPlay pair-verify (Ed25519 + X25519, AES-CTR), as an Apple TV 3 does it.
//!
//! Step 1 (`01 00 00 00` + client X25519 + client Ed25519): reply our X25519 key and our
//! signature of (ours + theirs), AES-CTR encrypted with keys from the shared secret.
//! Step 2 (`00 00 00 00` + 64 bytes): the client's signature, encrypted after 64 bytes of the
//! same keystream; a mismatch is logged, not fatal (as in the Python receiver).

use aes::cipher::StreamCipher;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use x25519_dalek::{PublicKey, StaticSecret};

use crate::{ctr, sha512_16, Ctr};

#[derive(Default)]
pub struct PairVerify {
    ours: Option<StaticSecret>,
    theirs_x: [u8; 32],
    theirs_ed: [u8; 32],
    /// The shared secret, once step 1 ran.
    pub secret: Option<[u8; 32]>,
}

impl PairVerify {
    fn cipher(secret: &[u8; 32]) -> Ctr {
        ctr(
            &sha512_16(&[b"Pair-Verify-AES-Key", secret]),
            &sha512_16(&[b"Pair-Verify-AES-IV", secret]),
        )
    }

    /// A `POST /pair-verify` body; returns the reply body.
    pub fn handle(&mut self, key: &SigningKey, body: &[u8]) -> Vec<u8> {
        self.handle_with(key, body, rand::random())
    }

    fn handle_with(&mut self, key: &SigningKey, body: &[u8], our_secret: [u8; 32]) -> Vec<u8> {
        if body.len() == 68 && body[0] == 1 {
            self.theirs_x = body[4..36].try_into().unwrap();
            self.theirs_ed = body[36..68].try_into().unwrap();
            let ours = StaticSecret::from(our_secret);
            let ours_pub = PublicKey::from(&ours).to_bytes();
            let secret = ours.diffie_hellman(&PublicKey::from(self.theirs_x)).to_bytes();
            let mut msg = ours_pub.to_vec();
            msg.extend_from_slice(&self.theirs_x);
            let mut sig = key.sign(&msg).to_bytes();
            Self::cipher(&secret).apply_keystream(&mut sig);
            self.ours = Some(ours);
            self.secret = Some(secret);
            let mut out = ours_pub.to_vec();
            out.extend_from_slice(&sig);
            return out;
        }
        if let (true, Some(secret), Some(ours)) = (body.len() == 68 && body[0] == 0, self.secret, &self.ours) {
            let mut c = Self::cipher(&secret);
            c.apply_keystream(&mut [0u8; 64]); // the first 64 bytes signed our reply
            let mut sig: [u8; 64] = body[4..68].try_into().unwrap();
            c.apply_keystream(&mut sig);
            let mut msg = self.theirs_x.to_vec();
            msg.extend_from_slice(&PublicKey::from(ours).to_bytes());
            let ok = VerifyingKey::from_bytes(&self.theirs_ed)
                .is_ok_and(|k| k.verify(&msg, &Signature::from_bytes(&sig)).is_ok());
            if !ok {
                log::warn!("airplay: pair-verify signature mismatch (continuing)");
            }
        }
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{hex, vectors as v};

    #[test]
    fn matches_the_python_receiver() {
        let key = SigningKey::from_bytes(&std::array::from_fn(|i| i as u8));
        let mut p = PairVerify::default();
        let mut body = vec![1, 0, 0, 0];
        body.extend(hex(v::CLIENT_X));
        body.extend(hex(v::CLIENT_ED));
        assert_eq!(p.handle_with(&key, &body, [7; 32]), hex(v::REPLY1));
        assert_eq!(p.secret.unwrap().to_vec(), hex(v::SECRET));
        // A good step 2 verifies (no warning path); its reply is empty.
        assert!(p.handle(&key, &hex(v::BODY2)).is_empty());
        let msg = {
            let mut c = PairVerify::cipher(&p.secret.unwrap());
            c.apply_keystream(&mut [0u8; 64]);
            let mut sig: [u8; 64] = hex(v::BODY2)[4..].try_into().unwrap();
            c.apply_keystream(&mut sig);
            sig
        };
        let mut signed = hex(v::CLIENT_X);
        signed.extend_from_slice(&hex(v::REPLY1)[..32]);
        let client = VerifyingKey::from_bytes(&hex(v::CLIENT_ED).try_into().unwrap()).unwrap();
        assert!(client.verify(&signed, &Signature::from_bytes(&msg)).is_ok());
    }
}

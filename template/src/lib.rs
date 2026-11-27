// OotleGift — a gift card template for Tari Ootle.
//
// A creator locks TARI in a GiftCard component together with the SHA-256 hash
// of a redemption code and a message. Anyone who knows the code can redeem
// the full amount. If the card is never redeemed, the creator can reclaim
// the funds once the optional expiry epoch has been reached.
//
// SPDX-License-Identifier: MIT

use tari_template_lib::prelude::*;

#[template]
mod gift_card {
    use super::*;

    /// A single gift card holding TARI in a vault.
    pub struct GiftCard {
        /// The TARI locked in this card.
        funds: Vault,
        /// SHA-256 digest (32 bytes) of the redemption code (UTF-8).
        code_hash: Vec<u8>,
        /// A message from the creator to the recipient.
        message: String,
        /// Optional epoch at which the card expires and becomes reclaimable.
        expiry_epoch: Option<u64>,
        /// True once the card has been redeemed.
        redeemed: bool,
        /// True once the creator has reclaimed the funds after expiry.
        reclaimed: bool,
        /// Public key of the transaction signer that created the card.
        owner: RistrettoPublicKeyBytes,
    }

    impl GiftCard {
        /// Creates a new gift card from a bucket of TARI.
        ///
        /// # Panics
        ///
        /// Panics if the bucket is not TARI, is empty, if `code_hash` is not
        /// exactly 32 bytes, or if `expiry_epoch` is not in the future.
        pub fn new(
            funds: Bucket,
            code_hash: Vec<u8>,
            message: String,
            expiry_epoch: Option<u64>,
        ) -> Component<Self> {
            assert_eq!(
                funds.resource_address(),
                TARI_TOKEN,
                "Gift cards can only hold TARI"
            );
            assert!(
                !funds.amount().is_zero(),
                "Gift card must contain a positive amount of TARI"
            );
            assert_eq!(
                code_hash.len(),
                32,
                "code_hash must be a 32-byte SHA-256 digest"
            );
            if let Some(expiry) = expiry_epoch {
                assert!(
                    expiry > Consensus::current_epoch(),
                    "Expiry epoch must be in the future"
                );
            }

            let amount = funds.amount().to_string();
            let access_rules = ComponentAccessRules::new()
                // Anyone holding the code may redeem; views are public.
                .method("redeem", rule![allow_all])
                .method("balance", rule![allow_all])
                .method("status", rule![allow_all])
                .method("message", rule![allow_all])
                .method("expiry_epoch", rule![allow_all]);
            // `reclaim` intentionally has no allow-all rule: it stays
            // restricted to the component owner, and additionally checks the
            // transaction signer in code.

            let component = Component::new(Self {
                funds: Vault::from_bucket(funds),
                code_hash,
                message,
                expiry_epoch,
                redeemed: false,
                reclaimed: false,
                owner: CallerContext::transaction_signer_public_key(),
            })
            .with_access_rules(access_rules)
            .with_owner_rule(OwnerRule::OwnedBySigner)
            .create();

            emit_event(
                "GiftCardCreated",
                metadata![
                    "amount" => amount,
                    "has_expiry" => expiry_epoch.is_some().to_string(),
                ],
            );

            component
        }

        /// Redeems the card: if `code` hashes to the stored digest, the full
        /// balance is returned as a bucket. Callable by anyone.
        ///
        /// # Panics
        ///
        /// Panics if the card was already redeemed or reclaimed, or if the
        /// code is incorrect.
        pub fn redeem(&mut self, code: String) -> Bucket {
            assert!(!self.redeemed, "This gift card has already been redeemed");
            assert!(
                !self.reclaimed,
                "This gift card has already been reclaimed by its creator"
            );
            let digest = sha256(code.as_bytes());
            assert!(
                digest.as_slice() == self.code_hash.as_slice(),
                "Incorrect redemption code"
            );

            self.redeemed = true;
            let amount = self.funds.balance();
            emit_event(
                "GiftCardRedeemed",
                metadata!["amount" => amount.to_string()],
            );
            self.funds.withdraw(amount)
        }

        /// Reclaims the funds after expiry. Callable only by the creator,
        /// and only once the expiry epoch has been reached.
        ///
        /// # Panics
        ///
        /// Panics if the caller is not the creator, if the card was already
        /// redeemed or reclaimed, if the card has no expiry, or if the
        /// expiry epoch has not been reached yet.
        pub fn reclaim(&mut self) -> Bucket {
            assert_eq!(
                CallerContext::transaction_signer_public_key(),
                self.owner,
                "Only the creator can reclaim this gift card"
            );
            assert!(!self.redeemed, "This gift card has already been redeemed");
            assert!(
                !self.reclaimed,
                "This gift card has already been reclaimed by its creator"
            );
            let expiry = self
                .expiry_epoch
                .expect("This gift card has no expiry and cannot be reclaimed");
            assert!(
                Consensus::current_epoch() >= expiry,
                "This gift card has not expired yet"
            );

            self.reclaimed = true;
            let amount = self.funds.balance();
            emit_event(
                "GiftCardReclaimed",
                metadata!["amount" => amount.to_string()],
            );
            self.funds.withdraw(amount)
        }

        /// The amount of TARI currently locked in the card.
        pub fn balance(&self) -> Amount {
            self.funds.balance()
        }

        /// The card status: "active", "expired" (past its expiry epoch but
        /// not yet redeemed or reclaimed), "redeemed" or "reclaimed".
        pub fn status(&self) -> String {
            if self.redeemed {
                "redeemed".to_string()
            } else if self.reclaimed {
                "reclaimed".to_string()
            } else if let Some(expiry) = self.expiry_epoch {
                if Consensus::current_epoch() >= expiry {
                    "expired".to_string()
                } else {
                    "active".to_string()
                }
            } else {
                "active".to_string()
            }
        }

        /// The message attached to the card.
        pub fn message(&self) -> String {
            self.message.clone()
        }

        /// The expiry epoch of the card, if any.
        pub fn expiry_epoch(&self) -> Option<u64> {
            self.expiry_epoch
        }
    }

    /// SHA-256 (FIPS 180-4) implemented in pure Rust so the template has no
    /// extra dependencies. Verified against the standard test vectors by the
    /// engine tests (a card created with the known digest of a code can only
    /// be redeemed by that code).
    fn sha256(data: &[u8]) -> [u8; 32] {
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];

        let mut h: [u32; 8] = [
            0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
            0x5be0cd19,
        ];

        // Padding: message || 0x80 || zeros || 64-bit big-endian bit length.
        let bit_len = (data.len() as u64).wrapping_mul(8);
        let mut msg = data.to_vec();
        msg.push(0x80);
        while msg.len() % 64 != 56 {
            msg.push(0);
        }
        msg.extend_from_slice(&bit_len.to_be_bytes());

        for chunk in msg.chunks_exact(64) {
            let mut w = [0u32; 64];
            for i in 0..16 {
                w[i] = u32::from_be_bytes([
                    chunk[4 * i],
                    chunk[4 * i + 1],
                    chunk[4 * i + 2],
                    chunk[4 * i + 3],
                ]);
            }
            for i in 16..64 {
                let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
                let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
                w[i] = w[i - 16]
                    .wrapping_add(s0)
                    .wrapping_add(w[i - 7])
                    .wrapping_add(s1);
            }

            let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
                (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);

            for i in 0..64 {
                let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
                let ch = (e & f) ^ ((!e) & g);
                let temp1 = hh
                    .wrapping_add(s1)
                    .wrapping_add(ch)
                    .wrapping_add(K[i])
                    .wrapping_add(w[i]);
                let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
                let maj = (a & b) ^ (a & c) ^ (b & c);
                let temp2 = s0.wrapping_add(maj);
                hh = g;
                g = f;
                f = e;
                e = d.wrapping_add(temp1);
                d = c;
                c = b;
                b = a;
                a = temp1.wrapping_add(temp2);
            }

            h[0] = h[0].wrapping_add(a);
            h[1] = h[1].wrapping_add(b);
            h[2] = h[2].wrapping_add(c);
            h[3] = h[3].wrapping_add(d);
            h[4] = h[4].wrapping_add(e);
            h[5] = h[5].wrapping_add(f);
            h[6] = h[6].wrapping_add(g);
            h[7] = h[7].wrapping_add(hh);
        }

        let mut out = [0u8; 32];
        for (i, word) in h.iter().enumerate() {
            out[4 * i..4 * i + 4].copy_from_slice(&word.to_be_bytes());
        }
        out
    }
}

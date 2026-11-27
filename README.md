# OotleGift — Tari gift cards on Ootle 🎁

**OotleGift turns TARI into a gift card.** A creator locks TARI in a gift card
together with a message and the SHA-256 hash of a secret redemption code.
Anyone who knows the code can redeem the full amount — no wallet address
needed up front. If the gift is never claimed, the creator can reclaim the
funds after an optional expiry.

Built for the **Tari November Build Contest 2026 (MVP)**.

- **Live demo (browser simulation):** https://5himq.github.io/ootlegift/
- **Template:** Rust, `tari_template_lib` (Tari Ootle), in [`template/`](template/)
- **License:** MIT

## How it works

The `GiftCard` template (a Tari Ootle component) holds the gift's TARI in a
vault:

| Method | Who | What it does |
| --- | --- | --- |
| `new(funds, code_hash, message, expiry_epoch)` | creator | Creates a card from a TARI bucket. Only the 32-byte SHA-256 digest of the redemption code is stored — never the code itself. |
| `redeem(code) -> Bucket` | anyone | Hashes the code and compares it to the stored digest. On match, pays out the full balance. Fails if the code is wrong or the card was already redeemed/reclaimed. |
| `reclaim() -> Bucket` | creator only | Returns the funds to the creator, but only once the expiry epoch has been reached. Cards without an expiry can never be reclaimed. |
| `balance()` / `status()` / `message()` / `expiry_epoch()` | anyone | Public views. Status is `active`, `expired`, `redeemed` or `reclaimed`. |

SHA-256 is implemented in pure Rust inside the template (no extra
dependencies), and the engine tests verify it against reference digests: a
card created with the known hash of a code can only be redeemed by that code.

## Run the tests

The template is tested with `tari_template_test_tooling`, which runs the real
Ootle execution engine in-process:

```sh
cd template
cargo test
```

Covered cases: create + balance/status/message views, redeem with a wrong
code (rejected, funds stay locked), redeem with the correct code (pays out in
full), double redemption (rejected), reclaim before expiry (rejected),
reclaim after expiry by the creator (succeeds), reclaim by a non-owner
(rejected), and reclaim of a card with no expiry (rejected).

To build the WASM template:

```sh
cd template
cargo build --target wasm32-unknown-unknown --release
# -> target/wasm32-unknown-unknown/release/gift_card.wasm
```

## The demo

The hosted demo in [`docs/`](docs/) is a **browser simulation** of the
template: a single static HTML file (no build step) that re-implements the
template's rules one-for-one — SHA-256 via SubtleCrypto, one-time redemption,
creator-only reclaim after expiry — with state kept in local storage. It is
labelled as a simulation on the page; no real funds move there.

## On-chain status

The template is written against the public Esmeralda testnet toolchain and
verified by the engine tests above. A wallet account for publishing has been
created on Esmeralda with `tari_ootle_walletd`
(`component_961e9eba88e762a6f1e324f2ef41a2994aa6113a6e837954cdcf092d47231872`),
but publication is **pending a funded account**: the Esmeralda faucet is
currently empty (verified on-chain via the wallet daemon), and the
alternative funding route — mining testnet Minotari on L1, burning it and
claiming TARI on L2 — requires a synced Minotari node, which could not
establish stable L1 peer connections from the build environment used for
this entry. Once the account is funded, publishing is a single wallet
transaction; the template address and indexer links will be added here and
in the demo's "On-chain" section.

## Repository layout

```
template/   Rust Ootle template (GiftCard) + engine tests
docs/       Static demo (GitHub Pages) — plain HTML/CSS/JS
LICENSE     MIT
```

// Engine tests for the OotleGift GiftCard template, using
// tari_template_test_tooling (the real Ootle execution engine).
//
// SPDX-License-Identifier: MIT

use tari_engine_types::virtual_substate::{VirtualSubstate, VirtualSubstateId};
use tari_template_lib::types::{Amount, ComponentAddress};
use tari_template_lib::types::constants::TARI_TOKEN;
use tari_template_test_tooling::crypto::RistrettoSecretKey;
use tari_template_test_tooling::support::assert_error::assert_reject_reason;
use tari_template_test_tooling::transaction::{Workspace, args};
use tari_template_test_tooling::TemplateTest;

const TEMPLATE_NAME: &str = "GiftCard";
const CODE: &str = "GIFT-HAPPY-2026";
/// sha256("GIFT-HAPPY-2026"), computed with a reference implementation.
const CODE_HASH_HEX: &str = "3aed79fe6e66f31f1e337e968804c9c7a9366b81ba8f01e1905ae729395d45e6";
const WRONG_CODE: &str = "GIFT-WRONG-CODE";
const MESSAGE: &str = "Happy birthday! Coffee is on me.";
const AMOUNT: u64 = 500;
const START_EPOCH: u64 = 100;
const EXPIRY_EPOCH: u64 = 110;

fn hex_to_bytes(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn set_epoch(test: &mut TemplateTest, new_epoch: u64) {
    test.set_virtual_substate(
        VirtualSubstateId::CurrentEpoch,
        VirtualSubstate::CurrentEpoch(new_epoch),
    );
}

struct Setup {
    creator_account: ComponentAddress,
    creator_proof: tari_template_lib::types::NonFungibleAddress,
    creator_secret: <redacted>
    card: ComponentAddress,
}

/// Creates a funded creator account and a gift card holding AMOUNT TARI.
fn setup(test: &mut TemplateTest, expiry_epoch: Option<u64>) -> Setup {
    set_epoch(test, START_EPOCH);
    let template = test.get_template_address(TEMPLATE_NAME);
    let (creator_account, creator_proof, creator_secret) = test.create_funded_account();

    test.execute_expect_success(
        test.transaction()
            .call_method(creator_account, "withdraw", args![TARI_TOKEN, AMOUNT])
            .put_last_instruction_output_on_workspace("funds")
            .call_function(
                template,
                "new",
                args![
                    Workspace("funds"),
                    hex_to_bytes(CODE_HASH_HEX),
                    MESSAGE.to_string(),
                    expiry_epoch
                ],
            )
            .build_and_seal(&creator_secret),
        vec![creator_proof.clone()],
    );

    let (card, _) = test
        .read_only_state_store()
        .get_components_by_template_address(template)
        .unwrap()
        .remove(0);

    Setup {
        creator_account,
        creator_proof,
        creator_secret,
        card,
    }
}

fn card_balance(test: &mut TemplateTest, setup: &Setup) -> Amount {
    test.execute_expect_success(
        test.transaction()
            .call_method(setup.card, "balance", args![])
            .build_and_seal(&setup.creator_secret),
        vec![],
    );
    test.expect_return::<Amount>(0)
}

fn card_status(test: &mut TemplateTest, setup: &Setup) -> String {
    test.execute_expect_success(
        test.transaction()
            .call_method(setup.card, "status", args![])
            .build_and_seal(&setup.creator_secret),
        vec![],
    );
    test.expect_return::<String>(0)
}

fn account_balance(test: &mut TemplateTest, account: ComponentAddress, secret: &RistrettoSecretKey) -> Amount {
    test.execute_expect_success(
        test.transaction()
            .call_method(account, "balance", args![TARI_TOKEN])
            .build_and_seal(secret),
        vec![],
    );
    test.expect_return::<Amount>(0)
}

#[test]
fn create_and_balance() {
    let mut test = TemplateTest::my_crate();
    let setup = setup(&mut test, Some(EXPIRY_EPOCH));

    assert_eq!(card_balance(&mut test, &setup), Amount::from(AMOUNT));
    assert_eq!(card_status(&mut test, &setup), "active");

    test.execute_expect_success(
        test.transaction()
            .call_method(setup.card, "message", args![])
            .build_and_seal(&setup.creator_secret),
        vec![],
    );
    assert_eq!(test.expect_return::<String>(0), MESSAGE);
}

#[test]
fn redeem_wrong_code_fails() {
    let mut test = TemplateTest::my_crate();
    let setup = setup(&mut test, Some(EXPIRY_EPOCH));
    let (redeemer_account, _proof, redeemer_secret) = test.create_empty_account();

    let reason = test.execute_expect_failure(
        test.transaction()
            .call_method(setup.card, "redeem", args![WRONG_CODE.to_string()])
            .put_last_instruction_output_on_workspace("payout")
            .call_method(redeemer_account, "deposit", args![Workspace("payout")])
            .build_and_seal(&redeemer_secret),
        vec![],
    );
    assert_reject_reason(reason, "Incorrect redemption code");

    // Funds are still locked in the card.
    assert_eq!(card_balance(&mut test, &setup), Amount::from(AMOUNT));
    assert_eq!(card_status(&mut test, &setup), "active");
}

#[test]
fn redeem_correct_code_succeeds() {
    let mut test = TemplateTest::my_crate();
    let setup = setup(&mut test, Some(EXPIRY_EPOCH));
    let (redeemer_account, _proof, redeemer_secret) = test.create_empty_account();

    test.execute_expect_success(
        test.transaction()
            .call_method(setup.card, "redeem", args![CODE.to_string()])
            .put_last_instruction_output_on_workspace("payout")
            .call_method(redeemer_account, "deposit", args![Workspace("payout")])
            .build_and_seal(&redeemer_secret),
        vec![],
    );

    assert_eq!(
        account_balance(&mut test, redeemer_account, &redeemer_secret),
        Amount::from(AMOUNT)
    );
    assert_eq!(card_balance(&mut test, &setup), Amount::from(0u64));
    assert_eq!(card_status(&mut test, &setup), "redeemed");
}

#[test]
fn double_redeem_fails() {
    let mut test = TemplateTest::my_crate();
    let setup = setup(&mut test, Some(EXPIRY_EPOCH));
    let (redeemer_account, _proof, redeemer_secret) = test.create_empty_account();

    test.execute_expect_success(
        test.transaction()
            .call_method(setup.card, "redeem", args![CODE.to_string()])
            .put_last_instruction_output_on_workspace("payout")
            .call_method(redeemer_account, "deposit", args![Workspace("payout")])
            .build_and_seal(&redeemer_secret),
        vec![],
    );

    let reason = test.execute_expect_failure(
        test.transaction()
            .call_method(setup.card, "redeem", args![CODE.to_string()])
            .build_and_seal(&redeemer_secret),
        vec![],
    );
    assert_reject_reason(reason, "already been redeemed");
}

#[test]
fn reclaim_before_expiry_fails() {
    let mut test = TemplateTest::my_crate();
    let setup = setup(&mut test, Some(EXPIRY_EPOCH));

    let reason = test.execute_expect_failure(
        test.transaction()
            .call_method(setup.card, "reclaim", args![])
            .build_and_seal(&setup.creator_secret),
        vec![setup.creator_proof.clone()],
    );
    assert_reject_reason(reason, "not expired yet");
    assert_eq!(card_balance(&mut test, &setup), Amount::from(AMOUNT));
}

#[test]
fn reclaim_after_expiry_succeeds() {
    let mut test = TemplateTest::my_crate();
    let setup = setup(&mut test, Some(EXPIRY_EPOCH));

    let before = account_balance(&mut test, setup.creator_account, &setup.creator_secret);

    set_epoch(&mut test, EXPIRY_EPOCH);
    assert_eq!(card_status(&mut test, &setup), "expired");

    test.execute_expect_success(
        test.transaction()
            .call_method(setup.card, "reclaim", args![])
            .put_last_instruction_output_on_workspace("refund")
            .call_method(
                setup.creator_account,
                "deposit",
                args![Workspace("refund")],
            )
            .build_and_seal(&setup.creator_secret),
        vec![setup.creator_proof.clone()],
    );

    let after = account_balance(&mut test, setup.creator_account, &setup.creator_secret);
    assert_eq!(after, before + Amount::from(AMOUNT));
    assert_eq!(card_balance(&mut test, &setup), Amount::from(0u64));
    assert_eq!(card_status(&mut test, &setup), "reclaimed");
}

#[test]
fn reclaim_by_non_owner_fails() {
    let mut test = TemplateTest::my_crate();
    let setup = setup(&mut test, Some(EXPIRY_EPOCH));
    let (_stranger_account, _proof, stranger_secret) = test.create_funded_account();

    set_epoch(&mut test, EXPIRY_EPOCH);

    test.execute_expect_failure(
        test.transaction()
            .call_method(setup.card, "reclaim", args![])
            .build_and_seal(&stranger_secret),
        vec![],
    );

    // Funds are still locked in the card.
    assert_eq!(card_balance(&mut test, &setup), Amount::from(AMOUNT));
}

#[test]
fn reclaim_without_expiry_fails() {
    let mut test = TemplateTest::my_crate();
    let setup = setup(&mut test, None);

    let reason = test.execute_expect_failure(
        test.transaction()
            .call_method(setup.card, "reclaim", args![])
            .build_and_seal(&setup.creator_secret),
        vec![setup.creator_proof.clone()],
    );
    assert_reject_reason(reason, "no expiry");
}

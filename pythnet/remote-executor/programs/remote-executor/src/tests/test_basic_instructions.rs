use crate::{
    error::ExecutorError,
    tests::executor_simulator::{
        ExecutorAttack,
        VaaValidity,
    },
};

use super::executor_simulator::ExecutorBench;
use anchor_lang::{
    prelude::{
        Pubkey,
        Rent,
    },
    solana_program::{
        system_instruction::create_account,
        system_program,
    },
};
use solana_sdk::{
    native_token::LAMPORTS_PER_SOL,
    signature::Keypair,
    signer::Signer,
    system_instruction::transfer,
};

#[tokio::test]
async fn test_basic_instructions() {
    let mut bench = ExecutorBench::new();

    let emitter = Pubkey::new_unique();

    let executor_key = bench.get_executor_key(&emitter);

    let receiver = Keypair::new();
    let receiver2 = Keypair::new();
    let receiver3 = Keypair::new();
    let receiver4 = Keypair::new();

    let instruction1 = create_account(
        &executor_key,
        &receiver.pubkey(),
        Rent::default().minimum_balance(10),
        10,
        &Pubkey::new_unique(),
    );
    let instruction2 = create_account(
        &executor_key,
        &receiver2.pubkey(),
        Rent::default().minimum_balance(5),
        5,
        &Pubkey::new_unique(),
    );
    let instruction3 = transfer(
        &executor_key,
        &receiver3.pubkey(),
        Rent::default().minimum_balance(0),
    );
    let instruction4 = transfer(
        &executor_key,
        &receiver4.pubkey(),
        Rent::default().minimum_balance(0),
    );

    let vaa_account_create = bench.add_vaa_account(
        &emitter,
        &vec![instruction1, instruction2],
        VaaValidity::Valid,
    );
    let vaa_account_transfer2 =
        bench.add_vaa_account(&emitter, &vec![instruction4], VaaValidity::Valid);
    let vaa_account_transfer1 =
        bench.add_vaa_account(&emitter, &vec![instruction3], VaaValidity::Valid);

    let mut sim = bench.start().await;

    let pre_account = sim.get_account(receiver.pubkey()).await;
    let pre_account2 = sim.get_account(receiver2.pubkey()).await;
    let pre_account3 = sim.get_account(receiver3.pubkey()).await;
    let pre_account4 = sim.get_account(receiver4.pubkey()).await;
    assert_eq!(pre_account, None);
    assert_eq!(pre_account2, None);
    assert_eq!(pre_account3, None);
    assert_eq!(pre_account4, None);

    sim.airdrop(&executor_key, LAMPORTS_PER_SOL).await.unwrap();
    sim.execute_posted_vaa(
        &vaa_account_create,
        &vec![&receiver, &receiver2],
        ExecutorAttack::None,
    )
    .await
    .unwrap();

    let post_account = sim.get_account(receiver.pubkey()).await.unwrap();
    assert_eq!(post_account.lamports, Rent::default().minimum_balance(10));
    assert_eq!(post_account.data.len(), 10);

    let post_account2 = sim.get_account(receiver2.pubkey()).await.unwrap();
    assert_eq!(post_account2.lamports, Rent::default().minimum_balance(5));
    assert_eq!(post_account2.data.len(), 5);

    let claim_record_data = sim.get_claim_record(emitter).await;
    assert_eq!(claim_record_data.sequence, 1);

    sim.execute_posted_vaa(&vaa_account_transfer1, &vec![], ExecutorAttack::None)
        .await
        .unwrap();

    let post_account3 = sim.get_account(receiver3.pubkey()).await.unwrap();
    assert_eq!(post_account3.lamports, Rent::default().minimum_balance(0));
    assert_eq!(post_account3.data.len(), 0);
    assert_eq!(post_account3.owner, system_program::id());

    let claim_record_data = sim.get_claim_record(emitter).await;
    assert_eq!(claim_record_data.sequence, 3);

    // Replay attack
    assert_eq!(
        sim.execute_posted_vaa(&vaa_account_transfer1, &vec![], ExecutorAttack::None)
            .await
            .unwrap_err()
            .unwrap(),
        ExecutorError::NonIncreasingSequence.into()
    );

    let claim_record_data = sim.get_claim_record(emitter).await;
    assert_eq!(claim_record_data.sequence, 3);

    // Using a governance message with a lower sequence number attack
    assert_eq!(
        sim.execute_posted_vaa(&vaa_account_transfer2, &vec![], ExecutorAttack::None)
            .await
            .unwrap_err()
            .unwrap(),
        ExecutorError::NonIncreasingSequence.into()
    );
    let claim_record_data = sim.get_claim_record(emitter).await;
    assert_eq!(claim_record_data.sequence, 3);

    let post_account4 = sim.get_account(receiver4.pubkey()).await;
    assert_eq!(post_account4, None);
}
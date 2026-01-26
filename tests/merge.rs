#[cfg(test)]
mod tests {
    use litesvm::LiteSVM;
    use pinocchio::sysvars::{clock::CLOCK_ID as CLOCK_SYSVAR, rent::RENT_ID as RENT_SYSVAR};
    use solana_sdk::{
        account::Account,
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
        signature::{Keypair, Signer},
        transaction::Transaction,
    };
    use spl_associated_token_account::{ID as ATA_PROGRAM_ID, get_associated_token_address};
    use spl_token::ID as TOKEN_PROGRAM_ID;

    const PROGRAM_ID: Pubkey = Pubkey::new_from_array([
        0x0f, 0x1e, 0x6b, 0x14, 0x21, 0xc0, 0x4a, 0x07, 0x04, 0x31, 0x26, 0x5c, 0x19, 0xc5, 0xbb,
        0xee, 0x19, 0x92, 0xba, 0xe8, 0xaf, 0xd1, 0xcd, 0x07, 0x8e, 0xf8, 0xaf, 0x70, 0x47, 0xdc,
        0x11, 0xf7,
    ]);

    const STAKE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([
        6, 161, 216, 23, 145, 55, 84, 42, 152, 52, 55, 189, 254, 42, 122, 178, 85, 127, 83, 92,
        138, 120, 114, 43, 104, 164, 157, 192, 0, 0, 0, 0,
    ]);

    const VOTE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([
        7, 97, 72, 29, 53, 116, 116, 187, 124, 77, 118, 36, 235, 211, 189, 179, 216, 53, 94, 115,
        209, 16, 67, 252, 13, 163, 83, 128, 0, 0, 0, 0,
    ]);

    const STAKE_HISTORY_SYSVAR: Pubkey = Pubkey::new_from_array([
        6, 167, 213, 23, 25, 53, 132, 43, 117, 36, 142, 142, 69, 167, 74, 9, 0, 69, 35, 53, 181,
        203, 213, 234, 92, 199, 0, 0, 0, 0, 0, 0,
    ]);

    const STAKE_CONFIG: Pubkey = Pubkey::new_from_array([
        6, 161, 216, 23, 165, 2, 5, 11, 104, 7, 145, 230, 206, 95, 249, 248, 36, 45, 178, 171, 63,
        252, 207, 199, 82, 86, 83, 0, 0, 99, 1, 1,
    ]);

    const SYSTEM_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0; 32]);

    fn derive_pool_state_pda(initializer: &Pubkey, seed: u64) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[b"lst_pool", initializer.as_ref(), &seed.to_le_bytes()],
            &PROGRAM_ID,
        )
    }

    fn derive_lst_mint_pda(pool_state: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[b"lst_mint", pool_state.as_ref()], &PROGRAM_ID)
    }

    fn derive_stake_account_pda(pool_state: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[b"stake", pool_state.as_ref()], &PROGRAM_ID)
    }

    fn derive_deposit_stake_pda(pool_state: &Pubkey, depositor: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[b"stake", pool_state.as_ref(), depositor.as_ref()],
            &PROGRAM_ID,
        )
    }

    fn derive_reserve_stake_account_pda(pool_state: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[b"reserve_stake", pool_state.as_ref()], &PROGRAM_ID)
    }

    fn derive_ata(owner: &Pubkey, mint: &Pubkey) -> Pubkey {
        Pubkey::find_program_address(
            &[owner.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.as_ref()],
            &ATA_PROGRAM_ID,
        )
        .0
    }

    fn create_initialize_instruction_data(
        seed: u64,
        pool_bump: u8,
        mint_bump: u8,
        stake_bump: u8,
        reserve_bump: u8,
    ) -> Vec<u8> {
        let mut data = vec![0u8]; // Discriminator for Initialize
        data.extend_from_slice(&seed.to_le_bytes());
        data.push(pool_bump);
        data.push(mint_bump);
        data.push(stake_bump);
        data.push(reserve_bump);
        data
    }

    fn create_deposit_instruction_data(amount: u64) -> Vec<u8> {
        let mut data = vec![1u8]; // Discriminator for Deposit
        data.extend_from_slice(&amount.to_le_bytes());
        data
    }

    fn setup_svm() -> LiteSVM {
        let mut svm = LiteSVM::new().with_builtins().with_sigverify(false);

        svm.add_program_from_file(PROGRAM_ID, "target/deploy/liquid_staking_token.so")
            .expect("Failed to load program");

        svm
    }

    fn create_vote_account(svm: &mut LiteSVM, validator_identity: &Pubkey) -> Pubkey {
        let vote_keypair = Keypair::new();
        let vote_pubkey = vote_keypair.pubkey();

        let mut data = vec![0u8; 3762];
        data[0..4].copy_from_slice(&1u32.to_le_bytes());
        data[4..36].copy_from_slice(validator_identity.as_ref());
        data[36..68].copy_from_slice(validator_identity.as_ref());

        svm.set_account(
            vote_pubkey,
            Account {
                lamports: 10_000_000_000,
                data,
                owner: VOTE_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            }
            .into(),
        );

        vote_pubkey
    }

    fn print_transaction_logs(
        result: &Result<
            litesvm::types::TransactionMetadata,
            litesvm::types::FailedTransactionMetadata,
        >,
    ) {
        match result {
            Ok(meta) => {
                eprintln!("\n=== Transaction Succeeded ===");
                for log in &meta.logs {
                    eprintln!("  {}", log);
                }
            }
            Err(err) => {
                eprintln!("\n=== Transaction Failed ===");
                eprintln!("Error: {:?}", err.err);
                for log in &err.meta.logs {
                    eprintln!("  {}", log);
                }
            }
        }
    }

    /// Helper to initialize a pool and return all the PDAs
    fn initialize_pool(
        svm: &mut LiteSVM,
    ) -> (Keypair, Pubkey, Pubkey, Pubkey, Pubkey, Pubkey, u64) {
        let initializer = Keypair::new();
        svm.airdrop(&initializer.pubkey(), 2_000_000_000).unwrap();

        let validator_identity = Keypair::new();
        let validator_vote = create_vote_account(svm, &validator_identity.pubkey());

        let seed = 12345u64;

        let (pool_state_pda, pool_bump) = derive_pool_state_pda(&initializer.pubkey(), seed);
        let (lst_mint_pda, mint_bump) = derive_lst_mint_pda(&pool_state_pda);
        let (stake_account_pda, stake_bump) = derive_stake_account_pda(&pool_state_pda);
        let (reserve_stake_pda, reserve_bump) = derive_reserve_stake_account_pda(&pool_state_pda);
        let initializer_lst_ata =
        get_associated_token_address(&initializer.pubkey(), &lst_mint_pda);

        let instruction_data = create_initialize_instruction_data(
            seed,
            pool_bump,
            mint_bump,
            stake_bump,
            reserve_bump,
        );

        let instruction = Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(initializer.pubkey(), true), // initializer
                AccountMeta::new(initializer_lst_ata, false), // initializer_lst_ata
                AccountMeta::new(pool_state_pda, false),      // pool_state
                AccountMeta::new(lst_mint_pda, false),        // lst_mint
                AccountMeta::new(stake_account_pda, false),   // stake_account
                AccountMeta::new(reserve_stake_pda, false),   // reserve_stake
                AccountMeta::new_readonly(validator_vote, false), // validator_vote
                AccountMeta::new_readonly(CLOCK_SYSVAR.into(), false), // clock
                AccountMeta::new_readonly(RENT_SYSVAR.into(), false), // rent
                AccountMeta::new_readonly(STAKE_HISTORY_SYSVAR, false), // stake_history
                AccountMeta::new_readonly(STAKE_CONFIG, false), // stake_config
                AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false), // system_program
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false), // token_program
                AccountMeta::new_readonly(STAKE_PROGRAM_ID, false), // stake_program
                AccountMeta::new_readonly(ATA_PROGRAM_ID, false), // ata_program
            ],
            data: instruction_data,
        };

        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&initializer.pubkey()),
            &[&initializer],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(transaction);
        print_transaction_logs(&result);
        assert!(result.is_ok(), "Initialize should succeed");
        let reserve_after = svm.get_account(&reserve_stake_pda).unwrap();
        println!(
            "Reserve stake lamports after init: {}",
            reserve_after.lamports
        );
        println!("=== Pool Initialized Successfully ===");

        (
            initializer,
            pool_state_pda,
            lst_mint_pda,
            stake_account_pda,
            reserve_stake_pda,
            validator_vote,
            seed,
        )
    }

    #[test]
    fn test_merge_reserve_success() {
        let mut svm = setup_svm();

        // 1. Initialize pool (creates pool_stake delegated, reserve_stake created but empty)
        let (initializer, pool_state_pda, _, pool_stake_pda, reserve_stake_pda, validator_vote, _) =
            initialize_pool(&mut svm);

        // 2. Add lamports to reserve_stake (simulating deposits)
        svm.airdrop(&reserve_stake_pda, 2_000_000_000).unwrap();

        // 3. Initialize and delegate reserve_stake
        let crank = Keypair::new();
        svm.airdrop(&crank.pubkey(), 1_000_000_000).unwrap();

        let init_reserve_ix = Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(pool_state_pda, false), // pool_state
                AccountMeta::new_readonly(pool_stake_pda, false), // pool_stake
                AccountMeta::new(reserve_stake_pda, false), // reserve_stake
                AccountMeta::new_readonly(validator_vote, false), // validator_vote
                AccountMeta::new_readonly(CLOCK_SYSVAR.into(), false), // clock
                AccountMeta::new_readonly(RENT_SYSVAR.into(), false), // rent
                AccountMeta::new_readonly(STAKE_HISTORY_SYSVAR, false), // stake_history
                AccountMeta::new_readonly(STAKE_CONFIG, false), // stake_config
                AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false), // system_program
                AccountMeta::new_readonly(STAKE_PROGRAM_ID, false), // stake_program
            ],
            data: vec![2u8], // InitializeReserve discriminator
        };

        let tx = Transaction::new_signed_with_payer(
            &[init_reserve_ix],
            Some(&crank.pubkey()),
            &[&crank],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_ok(), "InitializeReserve should succeed");

        println!("\n=== Reserve Initialized & Delegated ===");

        // 4. Warp forward to next epoch so both stakes become active
        // Stakes need to be in the same state (both active) to merge
        let slots_per_epoch = 432_000; // mainnet default, LiteSVM might differ
        svm.warp_to_slot(slots_per_epoch * 2); // warp 2 epochs forward to be safe

        // 5. Call MergeReserve
        let pool_stake_before = svm.get_account(&pool_stake_pda).unwrap();
        let reserve_stake_before = svm.get_account(&reserve_stake_pda).unwrap();

        println!("\n=== Before Merge ===");
        println!("  Pool stake lamports: {}", pool_stake_before.lamports);
        println!(
            "  Reserve stake lamports: {}",
            reserve_stake_before.lamports
        );

        let merge_ix = Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(pool_state_pda, false),    // pool_state
                AccountMeta::new(pool_stake_pda, false),    // pool_stake (destination)
                AccountMeta::new(reserve_stake_pda, false), // reserve_stake (source)
                AccountMeta::new_readonly(CLOCK_SYSVAR.into(), false), // clock
                AccountMeta::new_readonly(STAKE_HISTORY_SYSVAR, false), // stake_history
                AccountMeta::new_readonly(STAKE_PROGRAM_ID, false), // stake_program
            ],
            data: vec![3u8], // MergeReserve discriminator
        };

        let tx = Transaction::new_signed_with_payer(
            &[merge_ix],
            Some(&crank.pubkey()),
            &[&crank],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_ok(), "MergeReserve should succeed");

        // 6. Verify merge results
        let pool_stake_after = svm.get_account(&pool_stake_pda).unwrap();
        let reserve_stake_after = svm.get_account(&reserve_stake_pda);

        println!("\n=== After Merge ===");
        println!("  Pool stake lamports: {}", pool_stake_after.lamports);

        // Reserve should be closed (absorbed into pool_stake)
        match reserve_stake_after {
            Some(acc) => {
                println!("  Reserve stake lamports: {}", acc.lamports);
                println!("  Reserve stake owner: {:?}", acc.owner);
                // After merge, reserve should be empty/system-owned
                assert_eq!(
                    acc.lamports, 0,
                    "Reserve should have 0 lamports after merge"
                );
            }
            None => {
                println!("  Reserve stake: CLOSED");
            }
        }

        // Pool stake should have absorbed reserve's lamports
        let expected_lamports = pool_stake_before.lamports + reserve_stake_before.lamports;
        assert_eq!(
            pool_stake_after.lamports, expected_lamports,
            "Pool stake should have absorbed reserve lamports"
        );

        println!("\n=== Merge Verified Successfully ===");
    }

    #[test]
fn test_merge_reserve_before_initialized_fails() {
    let mut svm = setup_svm();

    // Initialize pool (reserve is created but NOT initialized/delegated)
    let (_, pool_state_pda, _, pool_stake_pda, reserve_stake_pda, _, _) =
        initialize_pool(&mut svm);

    // Add lamports to reserve but DON'T call InitializeReserve
    svm.airdrop(&reserve_stake_pda, 2_000_000_000).unwrap();

    let crank = Keypair::new();
    svm.airdrop(&crank.pubkey(), 1_000_000_000).unwrap();

    // Try to merge without initializing reserve first
    let merge_ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(pool_state_pda, false),
            AccountMeta::new(pool_stake_pda, false),
            AccountMeta::new(reserve_stake_pda, false),
            AccountMeta::new_readonly(CLOCK_SYSVAR.into(), false),
            AccountMeta::new_readonly(STAKE_HISTORY_SYSVAR, false),
            AccountMeta::new_readonly(STAKE_PROGRAM_ID, false),
        ],
        data: vec![3u8], // MergeReserve discriminator
    };

    let tx = Transaction::new_signed_with_payer(
        &[merge_ix],
        Some(&crank.pubkey()),
        &[&crank],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    print_transaction_logs(&result);

    assert!(
        result.is_err(),
        "Merge should fail when reserve is not initialized/delegated"
    );

    println!("\n=== Test Passed: Merge Before Initialize Rejected ===");
}

#[test]
fn test_double_merge_fails() {
    let mut svm = setup_svm();

    // Initialize pool
    let (_, pool_state_pda, _, pool_stake_pda, reserve_stake_pda, validator_vote, _) =
        initialize_pool(&mut svm);

    // Add lamports to reserve
    svm.airdrop(&reserve_stake_pda, 2_000_000_000).unwrap();

    let crank = Keypair::new();
    svm.airdrop(&crank.pubkey(), 1_000_000_000).unwrap();

    // Initialize reserve
    let init_reserve_ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(pool_state_pda, false),
            AccountMeta::new_readonly(pool_stake_pda, false),
            AccountMeta::new(reserve_stake_pda, false),
            AccountMeta::new_readonly(validator_vote, false),
            AccountMeta::new_readonly(CLOCK_SYSVAR.into(), false),
            AccountMeta::new_readonly(RENT_SYSVAR.into(), false),
            AccountMeta::new_readonly(STAKE_HISTORY_SYSVAR, false),
            AccountMeta::new_readonly(STAKE_CONFIG, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            AccountMeta::new_readonly(STAKE_PROGRAM_ID, false),
        ],
        data: vec![2u8],
    };

    let tx = Transaction::new_signed_with_payer(
        &[init_reserve_ix],
        Some(&crank.pubkey()),
        &[&crank],
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).unwrap();

    // Warp forward so stakes become active
    let slots_per_epoch = 432_000;
    svm.warp_to_slot(slots_per_epoch * 2);

    let merge_ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(pool_state_pda, false),
            AccountMeta::new(pool_stake_pda, false),
            AccountMeta::new(reserve_stake_pda, false),
            AccountMeta::new_readonly(CLOCK_SYSVAR.into(), false),
            AccountMeta::new_readonly(STAKE_HISTORY_SYSVAR, false),
            AccountMeta::new_readonly(STAKE_PROGRAM_ID, false),
        ],
        data: vec![3u8],
    };

    // First merge should succeed
    let tx1 = Transaction::new_signed_with_payer(
        &[merge_ix.clone()],
        Some(&crank.pubkey()),
        &[&crank],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx1);
    print_transaction_logs(&result);
    assert!(result.is_ok(), "First merge should succeed");

    // Verify reserve was absorbed
    let reserve_after_first = svm.get_account(&reserve_stake_pda);
    eprintln!("\n=== After First Merge ===");
    match &reserve_after_first {
        Some(acc) => eprintln!("  Reserve lamports: {}, owner: {:?}", acc.lamports, acc.owner),
        None => eprintln!("  Reserve: CLOSED"),
    }

    // Second merge should fail
    let tx2 = Transaction::new_signed_with_payer(
        &[merge_ix],
        Some(&crank.pubkey()),
        &[&crank],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx2);
    print_transaction_logs(&result);

    assert!(
        result.is_err(),
        "Second merge should fail - reserve already absorbed"
    );

    println!("\n=== Test Passed: Double Merge Rejected ===");
}

#[test]
fn test_merge_reserve_empty_fails() {
    let mut svm = setup_svm();

    // Initialize pool
    let (_, pool_state_pda, _, pool_stake_pda, reserve_stake_pda, _, _) =
        initialize_pool(&mut svm);

    // DON'T add any lamports to reserve - it stays empty

    let crank = Keypair::new();
    svm.airdrop(&crank.pubkey(), 1_000_000_000).unwrap();

    let reserve_before = svm.get_account(&reserve_stake_pda).unwrap();
    eprintln!("Reserve lamports before merge attempt: {}", reserve_before.lamports);

    let merge_ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(pool_state_pda, false),
            AccountMeta::new(pool_stake_pda, false),
            AccountMeta::new(reserve_stake_pda, false),
            AccountMeta::new_readonly(CLOCK_SYSVAR.into(), false),
            AccountMeta::new_readonly(STAKE_HISTORY_SYSVAR, false),
            AccountMeta::new_readonly(STAKE_PROGRAM_ID, false),
        ],
        data: vec![3u8],
    };

    let tx = Transaction::new_signed_with_payer(
        &[merge_ix],
        Some(&crank.pubkey()),
        &[&crank],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    print_transaction_logs(&result);

    assert!(
        result.is_err(),
        "Merge should fail when reserve has 0 lamports"
    );

    println!("\n=== Test Passed: Empty Reserve Merge Rejected ===");
}

#[test]
fn test_merge_wrong_pool_stake_fails() {
    let mut svm = setup_svm();

    // Initialize pool
    let (_, pool_state_pda, _, _pool_stake_pda, reserve_stake_pda, validator_vote, _) =
        initialize_pool(&mut svm);

    // Add lamports and initialize reserve
    svm.airdrop(&reserve_stake_pda, 2_000_000_000).unwrap();

    let crank = Keypair::new();
    svm.airdrop(&crank.pubkey(), 1_000_000_000).unwrap();

    let init_reserve_ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(pool_state_pda, false),
            AccountMeta::new_readonly(_pool_stake_pda, false),
            AccountMeta::new(reserve_stake_pda, false),
            AccountMeta::new_readonly(validator_vote, false),
            AccountMeta::new_readonly(CLOCK_SYSVAR.into(), false),
            AccountMeta::new_readonly(RENT_SYSVAR.into(), false),
            AccountMeta::new_readonly(STAKE_HISTORY_SYSVAR, false),
            AccountMeta::new_readonly(STAKE_CONFIG, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            AccountMeta::new_readonly(STAKE_PROGRAM_ID, false),
        ],
        data: vec![2u8],
    };

    let tx = Transaction::new_signed_with_payer(
        &[init_reserve_ix],
        Some(&crank.pubkey()),
        &[&crank],
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).unwrap();

    // Warp forward
    let slots_per_epoch = 432_000;
    svm.warp_to_slot(slots_per_epoch * 2);

    // Create a FAKE pool stake account
    let fake_pool_stake = Keypair::new();
    svm.set_account(
        fake_pool_stake.pubkey(),
        Account {
            lamports: 2_000_000_000,
            data: vec![0u8; 200], // Fake stake data
            owner: STAKE_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        }
        .into(),
    );

    let merge_ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(pool_state_pda, false),
            AccountMeta::new(fake_pool_stake.pubkey(), false), // WRONG pool stake
            AccountMeta::new(reserve_stake_pda, false),
            AccountMeta::new_readonly(CLOCK_SYSVAR.into(), false),
            AccountMeta::new_readonly(STAKE_HISTORY_SYSVAR, false),
            AccountMeta::new_readonly(STAKE_PROGRAM_ID, false),
        ],
        data: vec![3u8],
    };

    let tx = Transaction::new_signed_with_payer(
        &[merge_ix],
        Some(&crank.pubkey()),
        &[&crank],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    print_transaction_logs(&result);

    assert!(
        result.is_err(),
        "Merge with wrong pool stake should fail"
    );

    println!("\n=== Test Passed: Wrong Pool Stake Rejected ===");
}

#[test]
fn test_merge_wrong_reserve_stake_fails() {
    let mut svm = setup_svm();

    // Initialize pool
    let (_, pool_state_pda, _, pool_stake_pda, reserve_stake_pda, validator_vote, _) =
        initialize_pool(&mut svm);

    // Add lamports and initialize real reserve
    svm.airdrop(&reserve_stake_pda, 2_000_000_000).unwrap();

    let crank = Keypair::new();
    svm.airdrop(&crank.pubkey(), 1_000_000_000).unwrap();

    let init_reserve_ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(pool_state_pda, false),
            AccountMeta::new_readonly(pool_stake_pda, false),
            AccountMeta::new(reserve_stake_pda, false),
            AccountMeta::new_readonly(validator_vote, false),
            AccountMeta::new_readonly(CLOCK_SYSVAR.into(), false),
            AccountMeta::new_readonly(RENT_SYSVAR.into(), false),
            AccountMeta::new_readonly(STAKE_HISTORY_SYSVAR, false),
            AccountMeta::new_readonly(STAKE_CONFIG, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            AccountMeta::new_readonly(STAKE_PROGRAM_ID, false),
        ],
        data: vec![2u8],
    };

    let tx = Transaction::new_signed_with_payer(
        &[init_reserve_ix],
        Some(&crank.pubkey()),
        &[&crank],
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).unwrap();

    // Warp forward
    let slots_per_epoch = 432_000;
    svm.warp_to_slot(slots_per_epoch * 2);

    // Create a FAKE reserve stake account
    let fake_reserve = Keypair::new();
    svm.set_account(
        fake_reserve.pubkey(),
        Account {
            lamports: 2_000_000_000,
            data: vec![0u8; 200], // Fake stake data
            owner: STAKE_PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        }
        .into(),
    );

    let merge_ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(pool_state_pda, false),
            AccountMeta::new(pool_stake_pda, false),
            AccountMeta::new(fake_reserve.pubkey(), false), // WRONG reserve
            AccountMeta::new_readonly(CLOCK_SYSVAR.into(), false),
            AccountMeta::new_readonly(STAKE_HISTORY_SYSVAR, false),
            AccountMeta::new_readonly(STAKE_PROGRAM_ID, false),
        ],
        data: vec![3u8],
    };

    let tx = Transaction::new_signed_with_payer(
        &[merge_ix],
        Some(&crank.pubkey()),
        &[&crank],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    print_transaction_logs(&result);

    assert!(
        result.is_err(),
        "Merge with wrong reserve stake should fail"
    );

    println!("\n=== Test Passed: Wrong Reserve Stake Rejected ===");
}
}

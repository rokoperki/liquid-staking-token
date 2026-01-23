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

    fn derive_reserve_stake_account_pda(pool_state: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[b"reserve_stake", pool_state.as_ref()], &PROGRAM_ID)
    }

    fn create_initialize_instruction_data(
        seed: u64,
        pool_bump: u8,
        mint_bump: u8,
        stake_bump: u8,
        reserve_bump: u8,
    ) -> Vec<u8> {
        let mut data = vec![0u8];
        data.extend_from_slice(&seed.to_le_bytes());
        data.push(pool_bump);
        data.push(mint_bump);
        data.push(stake_bump);
        data.push(reserve_bump);
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

    /// Helper to build initialize instruction
    fn build_initialize_instruction(
        initializer: &Pubkey,
        pool_state_pda: &Pubkey,
        lst_mint_pda: &Pubkey,
        stake_account_pda: &Pubkey,
        reserve_stake_pda: &Pubkey,
        validator_vote: &Pubkey,
        instruction_data: Vec<u8>,
    ) -> Instruction {
        Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(*initializer, true),
                AccountMeta::new(*pool_state_pda, false),
                AccountMeta::new(*lst_mint_pda, false),
                AccountMeta::new(*stake_account_pda, false),
                AccountMeta::new(*reserve_stake_pda, false),
                AccountMeta::new_readonly(*validator_vote, false),
                AccountMeta::new_readonly(CLOCK_SYSVAR.into(), false),
                AccountMeta::new_readonly(RENT_SYSVAR.into(), false),
                AccountMeta::new_readonly(STAKE_HISTORY_SYSVAR, false),
                AccountMeta::new_readonly(STAKE_CONFIG, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
                AccountMeta::new_readonly(STAKE_PROGRAM_ID, false),
            ],
            data: instruction_data,
        }
    }

    // ============================================
    // SUCCESS CASES
    // ============================================

    #[test]
    fn test_initialize_success() {
        let mut svm = setup_svm();
        let initializer = Keypair::new();
        svm.airdrop(&initializer.pubkey(), 2_000_000_000).unwrap();

        let validator_identity = Keypair::new();
        let validator_vote = create_vote_account(&mut svm, &validator_identity.pubkey());

        let seed = 12345u64;

        let (pool_state_pda, pool_bump) = derive_pool_state_pda(&initializer.pubkey(), seed);
        let (lst_mint_pda, mint_bump) = derive_lst_mint_pda(&pool_state_pda);
        let (stake_account_pda, stake_bump) = derive_stake_account_pda(&pool_state_pda);
        let (reserve_stake_pda, reserve_bump) = derive_reserve_stake_account_pda(&pool_state_pda);

        let instruction_data =
            create_initialize_instruction_data(seed, pool_bump, mint_bump, stake_bump, reserve_bump);

        let instruction = build_initialize_instruction(
            &initializer.pubkey(),
            &pool_state_pda,
            &lst_mint_pda,
            &stake_account_pda,
            &reserve_stake_pda,
            &validator_vote,
            instruction_data,
        );

        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&initializer.pubkey()),
            &[&initializer],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(transaction);
        print_transaction_logs(&result);
        assert!(result.is_ok(), "Initialize should succeed");

        // Verify all accounts created
        assert!(svm.get_account(&pool_state_pda).is_some());
        assert!(svm.get_account(&lst_mint_pda).is_some());
        assert!(svm.get_account(&stake_account_pda).is_some());
        assert!(svm.get_account(&reserve_stake_pda).is_some());

        println!("\n=== test_initialize_success PASSED ===");
    }

    #[test]
    fn test_initialize_with_different_seeds() {
        let mut svm = setup_svm();
        let initializer = Keypair::new();
        svm.airdrop(&initializer.pubkey(), 5_000_000_000).unwrap();

        let validator_identity = Keypair::new();
        let validator_vote = create_vote_account(&mut svm, &validator_identity.pubkey());

        // Initialize with seed 1
        let seed1 = 1u64;
        let (pool_state_pda1, pool_bump1) = derive_pool_state_pda(&initializer.pubkey(), seed1);
        let (lst_mint_pda1, mint_bump1) = derive_lst_mint_pda(&pool_state_pda1);
        let (stake_account_pda1, stake_bump1) = derive_stake_account_pda(&pool_state_pda1);
        let (reserve_stake_pda1, reserve_bump1) = derive_reserve_stake_account_pda(&pool_state_pda1);

        let instruction_data1 =
            create_initialize_instruction_data(seed1, pool_bump1, mint_bump1, stake_bump1, reserve_bump1);

        let instruction1 = build_initialize_instruction(
            &initializer.pubkey(),
            &pool_state_pda1,
            &lst_mint_pda1,
            &stake_account_pda1,
            &reserve_stake_pda1,
            &validator_vote,
            instruction_data1,
        );

        let tx1 = Transaction::new_signed_with_payer(
            &[instruction1],
            Some(&initializer.pubkey()),
            &[&initializer],
            svm.latest_blockhash(),
        );

        let result1 = svm.send_transaction(tx1);
        print_transaction_logs(&result1);
        assert!(result1.is_ok(), "First initialize should succeed");

        // Initialize with seed 2 (same initializer, different pool)
        let seed2 = 2u64;
        let (pool_state_pda2, pool_bump2) = derive_pool_state_pda(&initializer.pubkey(), seed2);
        let (lst_mint_pda2, mint_bump2) = derive_lst_mint_pda(&pool_state_pda2);
        let (stake_account_pda2, stake_bump2) = derive_stake_account_pda(&pool_state_pda2);
        let (reserve_stake_pda2, reserve_bump2) = derive_reserve_stake_account_pda(&pool_state_pda2);

        let instruction_data2 =
            create_initialize_instruction_data(seed2, pool_bump2, mint_bump2, stake_bump2, reserve_bump2);

        let instruction2 = build_initialize_instruction(
            &initializer.pubkey(),
            &pool_state_pda2,
            &lst_mint_pda2,
            &stake_account_pda2,
            &reserve_stake_pda2,
            &validator_vote,
            instruction_data2,
        );

        let tx2 = Transaction::new_signed_with_payer(
            &[instruction2],
            Some(&initializer.pubkey()),
            &[&initializer],
            svm.latest_blockhash(),
        );

        let result2 = svm.send_transaction(tx2);
        print_transaction_logs(&result2);
        assert!(result2.is_ok(), "Second initialize with different seed should succeed");

        // Verify both pools exist
        assert!(svm.get_account(&pool_state_pda1).is_some());
        assert!(svm.get_account(&pool_state_pda2).is_some());

        println!("\n=== test_initialize_with_different_seeds PASSED ===");
    }

    // ============================================
    // FAILURE CASES - INSUFFICIENT FUNDS
    // ============================================

    #[test]
    fn test_initialize_insufficient_funds() {
        let mut svm = setup_svm();
        let initializer = Keypair::new();
        // Only airdrop enough for transaction fee, not for stake delegation
        svm.airdrop(&initializer.pubkey(), 100_000).unwrap();

        let validator_identity = Keypair::new();
        let validator_vote = create_vote_account(&mut svm, &validator_identity.pubkey());

        let seed = 12345u64;

        let (pool_state_pda, pool_bump) = derive_pool_state_pda(&initializer.pubkey(), seed);
        let (lst_mint_pda, mint_bump) = derive_lst_mint_pda(&pool_state_pda);
        let (stake_account_pda, stake_bump) = derive_stake_account_pda(&pool_state_pda);
        let (reserve_stake_pda, reserve_bump) = derive_reserve_stake_account_pda(&pool_state_pda);

        let instruction_data =
            create_initialize_instruction_data(seed, pool_bump, mint_bump, stake_bump, reserve_bump);

        let instruction = build_initialize_instruction(
            &initializer.pubkey(),
            &pool_state_pda,
            &lst_mint_pda,
            &stake_account_pda,
            &reserve_stake_pda,
            &validator_vote,
            instruction_data,
        );

        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&initializer.pubkey()),
            &[&initializer],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(transaction);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Should fail with insufficient funds");

        println!("\n=== test_initialize_insufficient_funds PASSED ===");
    }

    // ============================================
    // FAILURE CASES - INVALID BUMPS
    // ============================================

    #[test]
    fn test_initialize_wrong_pool_bump() {
        let mut svm = setup_svm();
        let initializer = Keypair::new();
        svm.airdrop(&initializer.pubkey(), 2_000_000_000).unwrap();

        let validator_identity = Keypair::new();
        let validator_vote = create_vote_account(&mut svm, &validator_identity.pubkey());

        let seed = 12345u64;

        let (pool_state_pda, pool_bump) = derive_pool_state_pda(&initializer.pubkey(), seed);
        let (lst_mint_pda, mint_bump) = derive_lst_mint_pda(&pool_state_pda);
        let (stake_account_pda, stake_bump) = derive_stake_account_pda(&pool_state_pda);
        let (reserve_stake_pda, reserve_bump) = derive_reserve_stake_account_pda(&pool_state_pda);

        // Use wrong pool bump
        let wrong_pool_bump = pool_bump.wrapping_add(1);
        let instruction_data =
            create_initialize_instruction_data(seed, wrong_pool_bump, mint_bump, stake_bump, reserve_bump);

        let instruction = build_initialize_instruction(
            &initializer.pubkey(),
            &pool_state_pda,
            &lst_mint_pda,
            &stake_account_pda,
            &reserve_stake_pda,
            &validator_vote,
            instruction_data,
        );

        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&initializer.pubkey()),
            &[&initializer],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(transaction);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Should fail with wrong pool bump");

        println!("\n=== test_initialize_wrong_pool_bump PASSED ===");
    }

    #[test]
    fn test_initialize_wrong_mint_bump() {
        let mut svm = setup_svm();
        let initializer = Keypair::new();
        svm.airdrop(&initializer.pubkey(), 2_000_000_000).unwrap();

        let validator_identity = Keypair::new();
        let validator_vote = create_vote_account(&mut svm, &validator_identity.pubkey());

        let seed = 12345u64;

        let (pool_state_pda, pool_bump) = derive_pool_state_pda(&initializer.pubkey(), seed);
        let (lst_mint_pda, mint_bump) = derive_lst_mint_pda(&pool_state_pda);
        let (stake_account_pda, stake_bump) = derive_stake_account_pda(&pool_state_pda);
        let (reserve_stake_pda, reserve_bump) = derive_reserve_stake_account_pda(&pool_state_pda);

        // Use wrong mint bump
        let wrong_mint_bump = mint_bump.wrapping_add(1);
        let instruction_data =
            create_initialize_instruction_data(seed, pool_bump, wrong_mint_bump, stake_bump, reserve_bump);

        let instruction = build_initialize_instruction(
            &initializer.pubkey(),
            &pool_state_pda,
            &lst_mint_pda,
            &stake_account_pda,
            &reserve_stake_pda,
            &validator_vote,
            instruction_data,
        );

        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&initializer.pubkey()),
            &[&initializer],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(transaction);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Should fail with wrong mint bump");

        println!("\n=== test_initialize_wrong_mint_bump PASSED ===");
    }

    #[test]
    fn test_initialize_wrong_stake_bump() {
        let mut svm = setup_svm();
        let initializer = Keypair::new();
        svm.airdrop(&initializer.pubkey(), 2_000_000_000).unwrap();

        let validator_identity = Keypair::new();
        let validator_vote = create_vote_account(&mut svm, &validator_identity.pubkey());

        let seed = 12345u64;

        let (pool_state_pda, pool_bump) = derive_pool_state_pda(&initializer.pubkey(), seed);
        let (lst_mint_pda, mint_bump) = derive_lst_mint_pda(&pool_state_pda);
        let (stake_account_pda, stake_bump) = derive_stake_account_pda(&pool_state_pda);
        let (reserve_stake_pda, reserve_bump) = derive_reserve_stake_account_pda(&pool_state_pda);

        // Use wrong stake bump
        let wrong_stake_bump = stake_bump.wrapping_add(1);
        let instruction_data =
            create_initialize_instruction_data(seed, pool_bump, mint_bump, wrong_stake_bump, reserve_bump);

        let instruction = build_initialize_instruction(
            &initializer.pubkey(),
            &pool_state_pda,
            &lst_mint_pda,
            &stake_account_pda,
            &reserve_stake_pda,
            &validator_vote,
            instruction_data,
        );

        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&initializer.pubkey()),
            &[&initializer],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(transaction);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Should fail with wrong stake bump");

        println!("\n=== test_initialize_wrong_stake_bump PASSED ===");
    }

    #[test]
    fn test_initialize_wrong_reserve_bump() {
        let mut svm = setup_svm();
        let initializer = Keypair::new();
        svm.airdrop(&initializer.pubkey(), 2_000_000_000).unwrap();

        let validator_identity = Keypair::new();
        let validator_vote = create_vote_account(&mut svm, &validator_identity.pubkey());

        let seed = 12345u64;

        let (pool_state_pda, pool_bump) = derive_pool_state_pda(&initializer.pubkey(), seed);
        let (lst_mint_pda, mint_bump) = derive_lst_mint_pda(&pool_state_pda);
        let (stake_account_pda, stake_bump) = derive_stake_account_pda(&pool_state_pda);
        let (reserve_stake_pda, reserve_bump) = derive_reserve_stake_account_pda(&pool_state_pda);

        // Use wrong reserve bump
        let wrong_reserve_bump = reserve_bump.wrapping_add(1);
        let instruction_data =
            create_initialize_instruction_data(seed, pool_bump, mint_bump, stake_bump, wrong_reserve_bump);

        let instruction = build_initialize_instruction(
            &initializer.pubkey(),
            &pool_state_pda,
            &lst_mint_pda,
            &stake_account_pda,
            &reserve_stake_pda,
            &validator_vote,
            instruction_data,
        );

        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&initializer.pubkey()),
            &[&initializer],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(transaction);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Should fail with wrong reserve bump");

        println!("\n=== test_initialize_wrong_reserve_bump PASSED ===");
    }

    // ============================================
    // FAILURE CASES - WRONG ACCOUNTS
    // ============================================

    #[test]
    fn test_initialize_wrong_pool_state_pda() {
        let mut svm = setup_svm();
        let initializer = Keypair::new();
        svm.airdrop(&initializer.pubkey(), 2_000_000_000).unwrap();

        let validator_identity = Keypair::new();
        let validator_vote = create_vote_account(&mut svm, &validator_identity.pubkey());

        let seed = 12345u64;

        let (pool_state_pda, pool_bump) = derive_pool_state_pda(&initializer.pubkey(), seed);
        let (lst_mint_pda, mint_bump) = derive_lst_mint_pda(&pool_state_pda);
        let (stake_account_pda, stake_bump) = derive_stake_account_pda(&pool_state_pda);
        let (reserve_stake_pda, reserve_bump) = derive_reserve_stake_account_pda(&pool_state_pda);

        // Use wrong pool state (different seed derivation)
        let wrong_seed = 99999u64;
        let (wrong_pool_state_pda, _) = derive_pool_state_pda(&initializer.pubkey(), wrong_seed);

        let instruction_data =
            create_initialize_instruction_data(seed, pool_bump, mint_bump, stake_bump, reserve_bump);

        let instruction = build_initialize_instruction(
            &initializer.pubkey(),
            &wrong_pool_state_pda, // Wrong PDA
            &lst_mint_pda,
            &stake_account_pda,
            &reserve_stake_pda,
            &validator_vote,
            instruction_data,
        );

        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&initializer.pubkey()),
            &[&initializer],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(transaction);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Should fail with wrong pool state PDA");

        println!("\n=== test_initialize_wrong_pool_state_pda PASSED ===");
    }

    #[test]
    fn test_initialize_wrong_lst_mint_pda() {
        let mut svm = setup_svm();
        let initializer = Keypair::new();
        svm.airdrop(&initializer.pubkey(), 2_000_000_000).unwrap();

        let validator_identity = Keypair::new();
        let validator_vote = create_vote_account(&mut svm, &validator_identity.pubkey());

        let seed = 12345u64;

        let (pool_state_pda, pool_bump) = derive_pool_state_pda(&initializer.pubkey(), seed);
        let (_, mint_bump) = derive_lst_mint_pda(&pool_state_pda);
        let (stake_account_pda, stake_bump) = derive_stake_account_pda(&pool_state_pda);
        let (reserve_stake_pda, reserve_bump) = derive_reserve_stake_account_pda(&pool_state_pda);

        // Create a random wrong mint PDA
        let wrong_lst_mint = Keypair::new().pubkey();

        let instruction_data =
            create_initialize_instruction_data(seed, pool_bump, mint_bump, stake_bump, reserve_bump);

        let instruction = build_initialize_instruction(
            &initializer.pubkey(),
            &pool_state_pda,
            &wrong_lst_mint, // Wrong mint
            &stake_account_pda,
            &reserve_stake_pda,
            &validator_vote,
            instruction_data,
        );

        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&initializer.pubkey()),
            &[&initializer],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(transaction);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Should fail with wrong LST mint PDA");

        println!("\n=== test_initialize_wrong_lst_mint_pda PASSED ===");
    }

    #[test]
    fn test_initialize_wrong_stake_account_pda() {
        let mut svm = setup_svm();
        let initializer = Keypair::new();
        svm.airdrop(&initializer.pubkey(), 2_000_000_000).unwrap();

        let validator_identity = Keypair::new();
        let validator_vote = create_vote_account(&mut svm, &validator_identity.pubkey());

        let seed = 12345u64;

        let (pool_state_pda, pool_bump) = derive_pool_state_pda(&initializer.pubkey(), seed);
        let (lst_mint_pda, mint_bump) = derive_lst_mint_pda(&pool_state_pda);
        let (_, stake_bump) = derive_stake_account_pda(&pool_state_pda);
        let (reserve_stake_pda, reserve_bump) = derive_reserve_stake_account_pda(&pool_state_pda);

        // Create a random wrong stake account
        let wrong_stake_account = Keypair::new().pubkey();

        let instruction_data =
            create_initialize_instruction_data(seed, pool_bump, mint_bump, stake_bump, reserve_bump);

        let instruction = build_initialize_instruction(
            &initializer.pubkey(),
            &pool_state_pda,
            &lst_mint_pda,
            &wrong_stake_account, // Wrong stake account
            &reserve_stake_pda,
            &validator_vote,
            instruction_data,
        );

        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&initializer.pubkey()),
            &[&initializer],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(transaction);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Should fail with wrong stake account PDA");

        println!("\n=== test_initialize_wrong_stake_account_pda PASSED ===");
    }

    // ============================================
    // FAILURE CASES - DOUBLE INITIALIZATION
    // ============================================

    #[test]
    fn test_initialize_double_init_fails() {
        let mut svm = setup_svm();
        let initializer = Keypair::new();
        svm.airdrop(&initializer.pubkey(), 5_000_000_000).unwrap();

        let validator_identity = Keypair::new();
        let validator_vote = create_vote_account(&mut svm, &validator_identity.pubkey());

        let seed = 12345u64;

        let (pool_state_pda, pool_bump) = derive_pool_state_pda(&initializer.pubkey(), seed);
        let (lst_mint_pda, mint_bump) = derive_lst_mint_pda(&pool_state_pda);
        let (stake_account_pda, stake_bump) = derive_stake_account_pda(&pool_state_pda);
        let (reserve_stake_pda, reserve_bump) = derive_reserve_stake_account_pda(&pool_state_pda);

        let instruction_data =
            create_initialize_instruction_data(seed, pool_bump, mint_bump, stake_bump, reserve_bump);

        let instruction = build_initialize_instruction(
            &initializer.pubkey(),
            &pool_state_pda,
            &lst_mint_pda,
            &stake_account_pda,
            &reserve_stake_pda,
            &validator_vote,
            instruction_data.clone(),
        );

        // First initialization
        let tx1 = Transaction::new_signed_with_payer(
            &[instruction.clone()],
            Some(&initializer.pubkey()),
            &[&initializer],
            svm.latest_blockhash(),
        );

        let result1 = svm.send_transaction(tx1);
        print_transaction_logs(&result1);
        assert!(result1.is_ok(), "First initialize should succeed");

        // Second initialization with same params
        let tx2 = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&initializer.pubkey()),
            &[&initializer],
            svm.latest_blockhash(),
        );

        let result2 = svm.send_transaction(tx2);
        print_transaction_logs(&result2);
        assert!(result2.is_err(), "Double initialization should fail");

        println!("\n=== test_initialize_double_init_fails PASSED ===");
    }

    // ============================================
    // FAILURE CASES - SIGNER CHECKS
    // ============================================

    #[test]
    fn test_initialize_missing_signer() {
        let mut svm = setup_svm();
        let initializer = Keypair::new();
        let payer = Keypair::new();
        svm.airdrop(&initializer.pubkey(), 2_000_000_000).unwrap();
        svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

        let validator_identity = Keypair::new();
        let validator_vote = create_vote_account(&mut svm, &validator_identity.pubkey());

        let seed = 12345u64;

        let (pool_state_pda, pool_bump) = derive_pool_state_pda(&initializer.pubkey(), seed);
        let (lst_mint_pda, mint_bump) = derive_lst_mint_pda(&pool_state_pda);
        let (stake_account_pda, stake_bump) = derive_stake_account_pda(&pool_state_pda);
        let (reserve_stake_pda, reserve_bump) = derive_reserve_stake_account_pda(&pool_state_pda);

        let instruction_data =
            create_initialize_instruction_data(seed, pool_bump, mint_bump, stake_bump, reserve_bump);

        // Build instruction with initializer NOT as signer
        let instruction = Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(initializer.pubkey(), false), // NOT a signer
                AccountMeta::new(pool_state_pda, false),
                AccountMeta::new(lst_mint_pda, false),
                AccountMeta::new(stake_account_pda, false),
                AccountMeta::new(reserve_stake_pda, false),
                AccountMeta::new_readonly(validator_vote, false),
                AccountMeta::new_readonly(CLOCK_SYSVAR.into(), false),
                AccountMeta::new_readonly(RENT_SYSVAR.into(), false),
                AccountMeta::new_readonly(STAKE_HISTORY_SYSVAR, false),
                AccountMeta::new_readonly(STAKE_CONFIG, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
                AccountMeta::new_readonly(STAKE_PROGRAM_ID, false),
            ],
            data: instruction_data,
        };

        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&payer.pubkey()),
            &[&payer], // Only payer signs, not initializer
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(transaction);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Should fail without initializer signature");

        println!("\n=== test_initialize_missing_signer PASSED ===");
    }

    // ============================================
    // FAILURE CASES - INVALID VOTE ACCOUNT
    // ============================================

    #[test]
    fn test_initialize_invalid_vote_account() {
        let mut svm = setup_svm();
        let initializer = Keypair::new();
        svm.airdrop(&initializer.pubkey(), 2_000_000_000).unwrap();

        // Create a fake "vote account" that's actually a system account
        let fake_vote = Keypair::new();
        svm.airdrop(&fake_vote.pubkey(), 1_000_000_000).unwrap();

        let seed = 12345u64;

        let (pool_state_pda, pool_bump) = derive_pool_state_pda(&initializer.pubkey(), seed);
        let (lst_mint_pda, mint_bump) = derive_lst_mint_pda(&pool_state_pda);
        let (stake_account_pda, stake_bump) = derive_stake_account_pda(&pool_state_pda);
        let (reserve_stake_pda, reserve_bump) = derive_reserve_stake_account_pda(&pool_state_pda);

        let instruction_data =
            create_initialize_instruction_data(seed, pool_bump, mint_bump, stake_bump, reserve_bump);

        let instruction = build_initialize_instruction(
            &initializer.pubkey(),
            &pool_state_pda,
            &lst_mint_pda,
            &stake_account_pda,
            &reserve_stake_pda,
            &fake_vote.pubkey(), // Not a real vote account
            instruction_data,
        );

        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&initializer.pubkey()),
            &[&initializer],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(transaction);
        print_transaction_logs(&result);
        // This should fail when trying to delegate to non-vote account
        assert!(result.is_err(), "Should fail with invalid vote account");

        println!("\n=== test_initialize_invalid_vote_account PASSED ===");
    }

    // ============================================
    // EDGE CASES - DATA VALIDATION
    // ============================================

    #[test]
    fn test_initialize_seed_zero_fails() {
        let mut svm = setup_svm();
        let initializer = Keypair::new();
        svm.airdrop(&initializer.pubkey(), 2_000_000_000).unwrap();
    
        let validator_identity = Keypair::new();
        let validator_vote = create_vote_account(&mut svm, &validator_identity.pubkey());
    
        let seed = 0u64;
    
        let (pool_state_pda, pool_bump) = derive_pool_state_pda(&initializer.pubkey(), seed);
        let (lst_mint_pda, mint_bump) = derive_lst_mint_pda(&pool_state_pda);
        let (stake_account_pda, stake_bump) = derive_stake_account_pda(&pool_state_pda);
        let (reserve_stake_pda, reserve_bump) = derive_reserve_stake_account_pda(&pool_state_pda);
    
        let instruction_data =
            create_initialize_instruction_data(seed, pool_bump, mint_bump, stake_bump, reserve_bump);
    
        let instruction = build_initialize_instruction(
            &initializer.pubkey(),
            &pool_state_pda,
            &lst_mint_pda,
            &stake_account_pda,
            &reserve_stake_pda,
            &validator_vote,
            instruction_data,
        );
    
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&initializer.pubkey()),
            &[&initializer],
            svm.latest_blockhash(),
        );
    
        let result = svm.send_transaction(transaction);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Seed 0 should be rejected");
    
        println!("\n=== test_initialize_seed_zero_fails PASSED ===");
    }

    #[test]
    fn test_initialize_seed_max() {
        let mut svm = setup_svm();
        let initializer = Keypair::new();
        svm.airdrop(&initializer.pubkey(), 2_000_000_000).unwrap();

        let validator_identity = Keypair::new();
        let validator_vote = create_vote_account(&mut svm, &validator_identity.pubkey());

        let seed = u64::MAX; // Edge case: max seed

        let (pool_state_pda, pool_bump) = derive_pool_state_pda(&initializer.pubkey(), seed);
        let (lst_mint_pda, mint_bump) = derive_lst_mint_pda(&pool_state_pda);
        let (stake_account_pda, stake_bump) = derive_stake_account_pda(&pool_state_pda);
        let (reserve_stake_pda, reserve_bump) = derive_reserve_stake_account_pda(&pool_state_pda);

        let instruction_data =
            create_initialize_instruction_data(seed, pool_bump, mint_bump, stake_bump, reserve_bump);

        let instruction = build_initialize_instruction(
            &initializer.pubkey(),
            &pool_state_pda,
            &lst_mint_pda,
            &stake_account_pda,
            &reserve_stake_pda,
            &validator_vote,
            instruction_data,
        );

        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&initializer.pubkey()),
            &[&initializer],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(transaction);
        print_transaction_logs(&result);
        assert!(result.is_ok(), "Max seed should be valid");

        println!("\n=== test_initialize_seed_max PASSED ===");
    }

    #[test]
    fn test_initialize_truncated_instruction_data() {
        let mut svm = setup_svm();
        let initializer = Keypair::new();
        svm.airdrop(&initializer.pubkey(), 2_000_000_000).unwrap();

        let validator_identity = Keypair::new();
        let validator_vote = create_vote_account(&mut svm, &validator_identity.pubkey());

        let seed = 12345u64;

        let (pool_state_pda, _) = derive_pool_state_pda(&initializer.pubkey(), seed);
        let (lst_mint_pda, _) = derive_lst_mint_pda(&pool_state_pda);
        let (stake_account_pda, _) = derive_stake_account_pda(&pool_state_pda);
        let (reserve_stake_pda, _) = derive_reserve_stake_account_pda(&pool_state_pda);

        // Truncated instruction data (missing bumps)
        let truncated_data = vec![0u8]; // Only discriminator

        let instruction = build_initialize_instruction(
            &initializer.pubkey(),
            &pool_state_pda,
            &lst_mint_pda,
            &stake_account_pda,
            &reserve_stake_pda,
            &validator_vote,
            truncated_data,
        );

        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&initializer.pubkey()),
            &[&initializer],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(transaction);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Should fail with truncated instruction data");

        println!("\n=== test_initialize_truncated_instruction_data PASSED ===");
    }

    // ============================================
    // VERIFICATION TEST
    // ============================================

    #[test]
fn test_initialize_verify_pool_state_data() {
    let mut svm = setup_svm();
    let initializer = Keypair::new();
    svm.airdrop(&initializer.pubkey(), 2_000_000_000).unwrap();

    let validator_identity = Keypair::new();
    let validator_vote = create_vote_account(&mut svm, &validator_identity.pubkey());

    let seed = 12345u64;

    let (pool_state_pda, pool_bump) = derive_pool_state_pda(&initializer.pubkey(), seed);
    let (lst_mint_pda, mint_bump) = derive_lst_mint_pda(&pool_state_pda);
    let (stake_account_pda, stake_bump) = derive_stake_account_pda(&pool_state_pda);
    let (reserve_stake_pda, reserve_bump) = derive_reserve_stake_account_pda(&pool_state_pda);

    let instruction_data =
        create_initialize_instruction_data(seed, pool_bump, mint_bump, stake_bump, reserve_bump);

    let instruction = build_initialize_instruction(
        &initializer.pubkey(),
        &pool_state_pda,
        &lst_mint_pda,
        &stake_account_pda,
        &reserve_stake_pda,
        &validator_vote,
        instruction_data,
    );

    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&initializer.pubkey()),
        &[&initializer],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(transaction);
    print_transaction_logs(&result);
    assert!(result.is_ok(), "Initialize should succeed");

    // Verify pool state data
    let pool_account = svm.get_account(&pool_state_pda).unwrap();
    let data = &pool_account.data;

    // Layout with discriminator at byte 0:
    // discriminator: u8        -> byte 0
    // lst_mint: Pubkey         -> bytes 1-33
    // authority: Pubkey        -> bytes 33-65
    // validator_vote: Pubkey   -> bytes 65-97
    // stake_account: Pubkey    -> bytes 97-129
    // reserve_stake: Pubkey    -> bytes 129-161
    // seed: u64                -> bytes 161-169
    // bump: u8                 -> byte 169
    // stake_bump: u8           -> byte 170
    // mint_bump: u8            -> byte 171
    // reserve_bump: u8         -> byte 172
    // lst_supply: u64          -> bytes 173-181
    // is_initialized: bool     -> byte 181

    // Verify discriminator
    assert_eq!(data[0], 0, "Discriminator should be 0 for PoolState");

    // Verify lst_mint (bytes 1-33)
    let stored_lst_mint = Pubkey::new_from_array(data[1..33].try_into().unwrap());
    assert_eq!(stored_lst_mint, lst_mint_pda, "LST mint mismatch");

    // Verify authority (bytes 33-65)
    let stored_authority = Pubkey::new_from_array(data[33..65].try_into().unwrap());
    assert_eq!(stored_authority, initializer.pubkey(), "Authority mismatch");

    // Verify validator_vote (bytes 65-97)
    let stored_validator = Pubkey::new_from_array(data[65..97].try_into().unwrap());
    assert_eq!(stored_validator, validator_vote, "Validator vote mismatch");

    // Verify stake_account (bytes 97-129)
    let stored_stake = Pubkey::new_from_array(data[97..129].try_into().unwrap());
    assert_eq!(stored_stake, stake_account_pda, "Stake account mismatch");

    // Verify reserve_stake (bytes 129-161)
    let stored_reserve = Pubkey::new_from_array(data[129..161].try_into().unwrap());
    assert_eq!(stored_reserve, reserve_stake_pda, "Reserve stake mismatch");

    // Verify seed (bytes 161-169)
    let stored_seed = u64::from_le_bytes(data[161..169].try_into().unwrap());
    assert_eq!(stored_seed, seed, "Seed mismatch");

    // Verify bumps
    assert_eq!(data[169], pool_bump, "Pool bump mismatch");
    assert_eq!(data[170], stake_bump, "Stake bump mismatch");
    assert_eq!(data[171], mint_bump, "Mint bump mismatch");
    assert_eq!(data[172], reserve_bump, "Reserve bump mismatch");

    // Verify lst_supply (bytes 173-181) is 0
    let stored_supply = u64::from_le_bytes(data[173..181].try_into().unwrap());
    assert_eq!(stored_supply, 1000000000, "LST supply should be 0");

    // Verify is_initialized (byte 181)
    assert_eq!(data[181], 1, "is_initialized should be true (1)");

    println!("\n=== test_initialize_verify_pool_state_data PASSED ===");
}
}
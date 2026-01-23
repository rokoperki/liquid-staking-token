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
    use spl_associated_token_account::ID as ATA_PROGRAM_ID;
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

    fn create_withdraw_instruction_data(amount: u64, nonce: u64, user_stake_bump: u8) -> Vec<u8> {
        let mut data = vec![4u8]; // Discriminator for Withdraw (based on DISCRIMINATOR: u8 = 4)
        data.extend_from_slice(&amount.to_le_bytes());
        data.extend_from_slice(&nonce.to_le_bytes());
        data.push(user_stake_bump);
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

    fn create_withdraw_complete_instruction_data(nonce: u64, user_stake_bump: u8) -> Vec<u8> {
        let mut data = vec![5u8]; // Discriminator for WithdrawComplete
        data.extend_from_slice(&nonce.to_le_bytes());
        data.push(user_stake_bump);
        data
    }

    #[test]
    fn test_withdraw_complete_success() {
        let mut svm = setup_svm();

        // Initialize pool first
        let (_, pool_state_pda, lst_mint_pda, pool_stake_pda, reserve_stake_pda, validator_vote, _) =
            initialize_pool(&mut svm);

        // Create user
        let user = Keypair::new();
        let deposit_amount = 10_000_000_000u64; // 10 SOL
        svm.airdrop(&user.pubkey(), 20_000_000_000).unwrap();

        // Create user's LST ATA
        let user_lst_ata = derive_ata(&user.pubkey(), &lst_mint_pda);

        let create_ata_ix =
            spl_associated_token_account::instruction::create_associated_token_account(
                &user.pubkey(),
                &user.pubkey(),
                &lst_mint_pda,
                &TOKEN_PROGRAM_ID,
            );

        let tx = Transaction::new_signed_with_payer(
            &[create_ata_ix],
            Some(&user.pubkey()),
            &[&user],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx).expect("Should create ATA");

        // Deposit to get LST tokens
        let deposit_data = create_deposit_instruction_data(deposit_amount);

        let deposit_ix = Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(user.pubkey(), true),
                AccountMeta::new(pool_state_pda, false),
                AccountMeta::new_readonly(pool_stake_pda, false),
                AccountMeta::new(reserve_stake_pda, false),
                AccountMeta::new(lst_mint_pda, false),
                AccountMeta::new(user_lst_ata, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
                AccountMeta::new_readonly(STAKE_PROGRAM_ID, false),
                AccountMeta::new_readonly(ATA_PROGRAM_ID, false),
            ],
            data: deposit_data,
        };

        let tx = Transaction::new_signed_with_payer(
            &[deposit_ix],
            Some(&user.pubkey()),
            &[&user],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_ok(), "Deposit should succeed");

        // Initialize reserve
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

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_ok(), "InitializeReserve should succeed");

        // Warp to activate stakes
        let slots_per_epoch = 432_000;
        svm.warp_to_slot(slots_per_epoch * 2);

        // Merge reserve
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
        assert!(result.is_ok(), "MergeReserve should succeed");

        // Withdraw (starts cooldown)
        let withdraw_amount = 5_000_000_000u64;
        let nonce = 1u64;

        let (user_stake_pda, user_stake_bump) = Pubkey::find_program_address(
            &[
                b"withdraw",
                pool_state_pda.as_ref(),
                user.pubkey().as_ref(),
                &nonce.to_le_bytes(),
            ],
            &PROGRAM_ID,
        );

        let withdraw_data =
            create_withdraw_instruction_data(withdraw_amount, nonce, user_stake_bump);

        let withdraw_ix = Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(user.pubkey(), true),
                AccountMeta::new(pool_state_pda, false),
                AccountMeta::new(pool_stake_pda, false),
                AccountMeta::new_readonly(reserve_stake_pda, false),
                AccountMeta::new(user_stake_pda, false),
                AccountMeta::new(lst_mint_pda, false),
                AccountMeta::new(user_lst_ata, false),
                AccountMeta::new_readonly(CLOCK_SYSVAR.into(), false),
                AccountMeta::new_readonly(RENT_SYSVAR.into(), false),
                AccountMeta::new_readonly(STAKE_HISTORY_SYSVAR, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
                AccountMeta::new_readonly(STAKE_PROGRAM_ID, false),
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            ],
            data: withdraw_data,
        };

        let tx = Transaction::new_signed_with_payer(
            &[withdraw_ix],
            Some(&user.pubkey()),
            &[&user],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_ok(), "Withdraw should succeed");

        // Verify user stake account exists
        let user_stake_account = svm.get_account(&user_stake_pda).unwrap();
        let user_stake_lamports = user_stake_account.lamports;
        println!(
            "User stake lamports after withdraw: {}",
            user_stake_lamports
        );
        assert!(user_stake_lamports > 0, "User stake should have lamports");

        // Record user SOL balance before claim
        let user_sol_before = svm.get_account(&user.pubkey()).unwrap().lamports;
        println!("User SOL before claim: {}", user_sol_before);

        // Warp forward to complete cooldown (stake deactivation)
        svm.warp_to_slot(slots_per_epoch * 4);

        // Call WithdrawComplete (claim)
        let withdraw_complete_data =
            create_withdraw_complete_instruction_data(nonce, user_stake_bump);

        let withdraw_complete_ix = Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(user.pubkey(), true), // user (signer, receives SOL)
                AccountMeta::new_readonly(pool_state_pda, false), // pool_state
                AccountMeta::new(user_stake_pda, false), // user_stake
                AccountMeta::new_readonly(CLOCK_SYSVAR.into(), false), // clock
                AccountMeta::new_readonly(STAKE_HISTORY_SYSVAR, false), // stake_history
                AccountMeta::new_readonly(STAKE_PROGRAM_ID, false), // stake_program
            ],
            data: withdraw_complete_data,
        };

        let tx = Transaction::new_signed_with_payer(
            &[withdraw_complete_ix],
            Some(&user.pubkey()),
            &[&user],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_ok(), "WithdrawComplete should succeed");

        // Verify user received SOL
        let user_sol_after = svm.get_account(&user.pubkey()).unwrap().lamports;
        println!("User SOL after claim: {}", user_sol_after);

        let sol_received = user_sol_after.saturating_sub(user_sol_before);
        println!("SOL received from claim: {}", sol_received);

        // User should receive approximately the stake amount (minus tx fee)
        // The exact amount includes rent from the closed stake account
        assert!(
            sol_received > user_stake_lamports - 10_000, // Allow for tx fee
            "User should receive stake lamports"
        );

        // Verify user stake account is closed
        let user_stake_after = svm.get_account(&user_stake_pda);
        match user_stake_after {
            Some(acc) => {
                assert_eq!(acc.lamports, 0, "User stake should have 0 lamports");
                println!("User stake account: 0 lamports (closed)");
            }
            None => {
                println!("User stake account: CLOSED");
            }
        }

        println!("\n=== WithdrawComplete Test Passed ===");
    }

    #[test]
    fn test_withdraw_complete_before_cooldown_fails() {
        let mut svm = setup_svm();

        // Initialize pool
        let (_, pool_state_pda, lst_mint_pda, pool_stake_pda, reserve_stake_pda, validator_vote, _) =
            initialize_pool(&mut svm);

        // Create user and deposit
        let user = Keypair::new();
        let deposit_amount = 10_000_000_000u64;
        svm.airdrop(&user.pubkey(), 20_000_000_000).unwrap();

        let user_lst_ata = derive_ata(&user.pubkey(), &lst_mint_pda);

        let create_ata_ix =
            spl_associated_token_account::instruction::create_associated_token_account(
                &user.pubkey(),
                &user.pubkey(),
                &lst_mint_pda,
                &TOKEN_PROGRAM_ID,
            );

        let tx = Transaction::new_signed_with_payer(
            &[create_ata_ix],
            Some(&user.pubkey()),
            &[&user],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx).expect("Should create ATA");

        let deposit_data = create_deposit_instruction_data(deposit_amount);

        let deposit_ix = Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(user.pubkey(), true),
                AccountMeta::new(pool_state_pda, false),
                AccountMeta::new_readonly(pool_stake_pda, false),
                AccountMeta::new(reserve_stake_pda, false),
                AccountMeta::new(lst_mint_pda, false),
                AccountMeta::new(user_lst_ata, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
                AccountMeta::new_readonly(STAKE_PROGRAM_ID, false),
                AccountMeta::new_readonly(ATA_PROGRAM_ID, false),
            ],
            data: deposit_data,
        };

        let tx = Transaction::new_signed_with_payer(
            &[deposit_ix],
            Some(&user.pubkey()),
            &[&user],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx).expect("Deposit should succeed");

        // Initialize reserve and merge
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
        svm.send_transaction(tx)
            .expect("InitializeReserve should succeed");

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

        let tx = Transaction::new_signed_with_payer(
            &[merge_ix],
            Some(&crank.pubkey()),
            &[&crank],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx)
            .expect("MergeReserve should succeed");

        // Withdraw
        let withdraw_amount = 5_000_000_000u64;
        let nonce = 1u64;

        let (user_stake_pda, user_stake_bump) = Pubkey::find_program_address(
            &[
                b"withdraw",
                pool_state_pda.as_ref(),
                user.pubkey().as_ref(),
                &nonce.to_le_bytes(),
            ],
            &PROGRAM_ID,
        );

        let withdraw_data =
            create_withdraw_instruction_data(withdraw_amount, nonce, user_stake_bump);

        let withdraw_ix = Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(user.pubkey(), true),
                AccountMeta::new(pool_state_pda, false),
                AccountMeta::new(pool_stake_pda, false),
                AccountMeta::new_readonly(reserve_stake_pda, false),
                AccountMeta::new(user_stake_pda, false),
                AccountMeta::new(lst_mint_pda, false),
                AccountMeta::new(user_lst_ata, false),
                AccountMeta::new_readonly(CLOCK_SYSVAR.into(), false),
                AccountMeta::new_readonly(RENT_SYSVAR.into(), false),
                AccountMeta::new_readonly(STAKE_HISTORY_SYSVAR, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
                AccountMeta::new_readonly(STAKE_PROGRAM_ID, false),
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            ],
            data: withdraw_data,
        };

        let tx = Transaction::new_signed_with_payer(
            &[withdraw_ix],
            Some(&user.pubkey()),
            &[&user],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx).expect("Withdraw should succeed");

        // Try to claim immediately WITHOUT warping (cooldown not complete)
        let withdraw_complete_data =
            create_withdraw_complete_instruction_data(nonce, user_stake_bump);

        let withdraw_complete_ix = Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(user.pubkey(), true),
                AccountMeta::new_readonly(pool_state_pda, false),
                AccountMeta::new(user_stake_pda, false),
                AccountMeta::new_readonly(CLOCK_SYSVAR.into(), false),
                AccountMeta::new_readonly(STAKE_HISTORY_SYSVAR, false),
                AccountMeta::new_readonly(STAKE_PROGRAM_ID, false),
            ],
            data: withdraw_complete_data,
        };

        let tx = Transaction::new_signed_with_payer(
            &[withdraw_complete_ix],
            Some(&user.pubkey()),
            &[&user],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);

        // Should fail because stake is not fully deactivated
        assert!(
            result.is_err(),
            "WithdrawComplete should fail before cooldown completes"
        );

        println!("\n=== WithdrawComplete Before Cooldown Test Passed (correctly failed) ===");
    }
}

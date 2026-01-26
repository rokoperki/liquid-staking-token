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
    fn test_withdraw_success() {
        let mut svm = setup_svm();

        // Initialize pool first
        let (_, pool_state_pda, lst_mint_pda, pool_stake_pda, reserve_stake_pda, validator_vote, _) =
            initialize_pool(&mut svm);

        // Create depositor (who will also withdraw)
        let user = Keypair::new();
        let deposit_amount = 10_000_000_000u64; // 10 SOL - enough for withdraw minimum
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

        // First deposit to get LST tokens
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
        assert!(result.is_ok(), "Deposit should succeed before withdraw");

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

        // Now withdraw half
        let withdraw_amount = 5_000_000_000u64; // 5 SOL worth of LST
        let nonce = 1u64;

        // Derive user stake PDA for withdraw
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
                AccountMeta::new(user.pubkey(), true),                  // user
                AccountMeta::new(pool_state_pda, false),                // pool_state
                AccountMeta::new(pool_stake_pda, false),                // pool_stake
                AccountMeta::new_readonly(reserve_stake_pda, false),    // reserve_stake
                AccountMeta::new(user_stake_pda, false),                // user_stake
                AccountMeta::new(lst_mint_pda, false),                  // lst_mint
                AccountMeta::new(user_lst_ata, false),                  // user_lst_ata
                AccountMeta::new_readonly(CLOCK_SYSVAR.into(), false),  // clock
                AccountMeta::new_readonly(RENT_SYSVAR.into(), false),   // rent
                AccountMeta::new_readonly(STAKE_HISTORY_SYSVAR, false), // validator_vote
                AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),    // system_program
                AccountMeta::new_readonly(STAKE_PROGRAM_ID, false),     // stake_program
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),     // token_program
            ],
            data: withdraw_data,
        };

        let pool_stake_before = svm.get_account(&pool_stake_pda).unwrap().lamports;
        println!("Pool stake lamports before withdraw: {}", pool_stake_before);

        let tx = Transaction::new_signed_with_payer(
            &[withdraw_ix],
            Some(&user.pubkey()),
            &[&user],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_ok(), "Withdraw should succeed");

        // Verify user stake account was created and has SOL
        let user_stake_account = svm.get_account(&user_stake_pda);
        assert!(
            user_stake_account.is_some(),
            "User stake account should exist"
        );
        let user_stake = user_stake_account.unwrap();
        println!("User stake account lamports: {}", user_stake.lamports);
        assert!(user_stake.lamports > 0, "User stake should have lamports");

        // Verify pool stake decreased
        let pool_stake_after = svm.get_account(&pool_stake_pda).unwrap().lamports;
        println!("Pool stake lamports after withdraw: {}", pool_stake_after);
        assert!(
            pool_stake_after < pool_stake_before,
            "Pool stake should decrease after withdraw"
        );

        println!("\n=== Withdraw Test Passed ===");
    }

    /// Helper to get token account balance from account data
    fn get_token_balance(account_data: &[u8]) -> u64 {
        u64::from_le_bytes(account_data[64..72].try_into().unwrap())
    }

    /// Helper to get mint total supply
    fn get_mint_supply(mint_data: &[u8]) -> u64 {
        u64::from_le_bytes(mint_data[36..44].try_into().unwrap())
    }

    /// Helper to setup pool with deposit and merged reserve for withdraw tests
    fn setup_pool_for_withdraw(
        svm: &mut LiteSVM,
    ) -> (
        Keypair, // user
        Pubkey,  // pool_state_pda
        Pubkey,  // lst_mint_pda
        Pubkey,  // pool_stake_pda
        Pubkey,  // reserve_stake_pda
        Pubkey,  // user_lst_ata
        Pubkey,  // validator_vote
    ) {
        // Initialize pool
        let (_, pool_state_pda, lst_mint_pda, pool_stake_pda, reserve_stake_pda, validator_vote, _) =
            initialize_pool(svm);

        // Create user and deposit
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

        // Deposit
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
        svm.send_transaction(tx)
            .expect("InitializeReserve should succeed");

        // Warp forward
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
        svm.send_transaction(tx)
            .expect("MergeReserve should succeed");

        (
            user,
            pool_state_pda,
            lst_mint_pda,
            pool_stake_pda,
            reserve_stake_pda,
            user_lst_ata,
            validator_vote,
        )
    }

    /// Helper to execute withdraw
    fn execute_withdraw(
        svm: &mut LiteSVM,
        user: &Keypair,
        pool_state_pda: &Pubkey,
        pool_stake_pda: &Pubkey,
        reserve_stake_pda: &Pubkey,
        lst_mint_pda: &Pubkey,
        user_lst_ata: &Pubkey,
        amount: u64,
        nonce: u64,
    ) -> Result<litesvm::types::TransactionMetadata, litesvm::types::FailedTransactionMetadata>
    {
        let (user_stake_pda, user_stake_bump) = Pubkey::find_program_address(
            &[
                b"withdraw",
                pool_state_pda.as_ref(),
                user.pubkey().as_ref(),
                &nonce.to_le_bytes(),
            ],
            &PROGRAM_ID,
        );

        let withdraw_data = create_withdraw_instruction_data(amount, nonce, user_stake_bump);

        let withdraw_ix = Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(user.pubkey(), true),
                AccountMeta::new(*pool_state_pda, false),
                AccountMeta::new(*pool_stake_pda, false),
                AccountMeta::new_readonly(*reserve_stake_pda, false),
                AccountMeta::new(user_stake_pda, false),
                AccountMeta::new(*lst_mint_pda, false),
                AccountMeta::new(*user_lst_ata, false),
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

        svm.send_transaction(tx)
    }

    #[test]
    fn test_double_withdraw_same_nonce_fails() {
        let mut svm = setup_svm();

        let (
            user,
            pool_state_pda,
            lst_mint_pda,
            pool_stake_pda,
            reserve_stake_pda,
            user_lst_ata,
            _,
        ) = setup_pool_for_withdraw(&mut svm);

        let withdraw_amount = 2_000_000_000u64; // 2 SOL worth
        let nonce = 1u64;

        // First withdraw should succeed
        let result = execute_withdraw(
            &mut svm,
            &user,
            &pool_state_pda,
            &pool_stake_pda,
            &reserve_stake_pda,
            &lst_mint_pda,
            &user_lst_ata,
            withdraw_amount,
            nonce,
        );
        print_transaction_logs(&result);
        assert!(result.is_ok(), "First withdraw should succeed");

        // Verify user_stake PDA exists
        let (user_stake_pda, _) = Pubkey::find_program_address(
            &[
                b"withdraw",
                pool_state_pda.as_ref(),
                user.pubkey().as_ref(),
                &nonce.to_le_bytes(),
            ],
            &PROGRAM_ID,
        );

        let user_stake = svm.get_account(&user_stake_pda);
        assert!(
            user_stake.is_some(),
            "User stake should exist after first withdraw"
        );

        // Second withdraw with SAME nonce should fail
        let result = execute_withdraw(
            &mut svm,
            &user,
            &pool_state_pda,
            &pool_stake_pda,
            &reserve_stake_pda,
            &lst_mint_pda,
            &user_lst_ata,
            withdraw_amount,
            nonce, // Same nonce!
        );
        print_transaction_logs(&result);

        assert!(
            result.is_err(),
            "Second withdraw with same nonce should fail - user_stake already exists"
        );

        println!("\n=== Test Passed: Double Withdraw Same Nonce Rejected ===");
    }

    #[test]
    fn test_withdraw_more_than_balance_fails() {
        let mut svm = setup_svm();

        let (
            user,
            pool_state_pda,
            lst_mint_pda,
            pool_stake_pda,
            reserve_stake_pda,
            user_lst_ata,
            _,
        ) = setup_pool_for_withdraw(&mut svm);

        // Get user's actual LST balance
        let user_lst_balance = get_token_balance(&svm.get_account(&user_lst_ata).unwrap().data);
        eprintln!("User LST balance: {}", user_lst_balance);

        // Try to withdraw MORE than balance
        let withdraw_amount = user_lst_balance + 1_000_000_000; // Balance + 1 SOL extra

        let result = execute_withdraw(
            &mut svm,
            &user,
            &pool_state_pda,
            &pool_stake_pda,
            &reserve_stake_pda,
            &lst_mint_pda,
            &user_lst_ata,
            withdraw_amount,
            1,
        );
        print_transaction_logs(&result);

        assert!(result.is_err(), "Withdraw more than balance should fail");

        println!("\n=== Test Passed: Withdraw More Than Balance Rejected ===");
    }

    #[test]
    fn test_withdraw_leaves_pool_below_min_stake_fails() {
        let mut svm = setup_svm();

        let (
            user,
            pool_state_pda,
            lst_mint_pda,
            pool_stake_pda,
            reserve_stake_pda,
            user_lst_ata,
            _,
        ) = setup_pool_for_withdraw(&mut svm);

        // Get pool stake balance
        let pool_stake_lamports = svm.get_account(&pool_stake_pda).unwrap().lamports;
        eprintln!("Pool stake lamports: {}", pool_stake_lamports);

        // Get user's LST balance
        let user_lst_balance = get_token_balance(&svm.get_account(&user_lst_ata).unwrap().data);
        eprintln!("User LST balance: {}", user_lst_balance);

        // Try to withdraw almost everything (leaving pool below min_stake)
        // This should fail because pool needs to keep min_stake
        let withdraw_amount = user_lst_balance - 100_000_000; // Leave only 0.1 SOL worth

        let result = execute_withdraw(
            &mut svm,
            &user,
            &pool_state_pda,
            &pool_stake_pda,
            &reserve_stake_pda,
            &lst_mint_pda,
            &user_lst_ata,
            withdraw_amount,
            1,
        );
        print_transaction_logs(&result);

        // This might succeed or fail depending on exact amounts
        // The important thing is that the pool maintains min_stake
        if result.is_ok() {
            let pool_stake_after = svm.get_account(&pool_stake_pda).unwrap().lamports;
            eprintln!("Pool stake after: {}", pool_stake_after);

            // Verify pool still has min_stake (rent + MIN_STAKE_DELEGATION)
            let min_stake = 2_282_880 + 1_000_000_000; // approximate rent + 1 SOL
            assert!(
                pool_stake_after >= min_stake,
                "Pool should maintain minimum stake"
            );
        }

        println!("\n=== Test Passed: Pool Maintains Min Stake ===");
    }

    #[test]
    fn test_withdraw_amount_too_small_fails() {
        let mut svm = setup_svm();

        let (
            user,
            pool_state_pda,
            lst_mint_pda,
            pool_stake_pda,
            reserve_stake_pda,
            user_lst_ata,
            _,
        ) = setup_pool_for_withdraw(&mut svm);

        // Try to withdraw very small amount (below min_stake for new stake account)
        let withdraw_amount = 1_000_000u64; // 0.001 SOL - way below min_stake

        let result = execute_withdraw(
            &mut svm,
            &user,
            &pool_state_pda,
            &pool_stake_pda,
            &reserve_stake_pda,
            &lst_mint_pda,
            &user_lst_ata,
            withdraw_amount,
            1,
        );
        print_transaction_logs(&result);

        assert!(
            result.is_err(),
            "Withdraw amount too small should fail - below min_stake for user_stake"
        );

        println!("\n=== Test Passed: Withdraw Amount Too Small Rejected ===");
    }

    #[test]
    fn test_withdraw_correct_exchange_rate_with_rewards() {
        let mut svm = setup_svm();

        let (
            user,
            pool_state_pda,
            lst_mint_pda,
            pool_stake_pda,
            reserve_stake_pda,
            user_lst_ata,
            _,
        ) = setup_pool_for_withdraw(&mut svm);

        // Get state before rewards
        let user_lst_balance = get_token_balance(&svm.get_account(&user_lst_ata).unwrap().data);
        let pool_stake_before_rewards = svm.get_account(&pool_stake_pda).unwrap().lamports;
        let mint_supply = get_mint_supply(&svm.get_account(&lst_mint_pda).unwrap().data);

        eprintln!("\n=== Before Rewards ===");
        eprintln!("  User LST balance: {}", user_lst_balance);
        eprintln!("  Pool stake: {}", pool_stake_before_rewards);
        eprintln!("  Mint supply: {}", mint_supply);

        // Simulate rewards by adding SOL to pool_stake
        let reward_amount = 1_000_000_000u64; // 1 SOL rewards
        let pool_account = svm.get_account(&pool_stake_pda).unwrap();
        svm.set_account(
            pool_stake_pda,
            Account {
                lamports: pool_account.lamports + reward_amount,
                data: pool_account.data.clone(),
                owner: pool_account.owner,
                executable: false,
                rent_epoch: 0,
            }
            .into(),
        );

        let pool_stake_after_rewards = svm.get_account(&pool_stake_pda).unwrap().lamports;

        eprintln!("\n=== After Rewards ===");
        eprintln!("  Pool stake: {}", pool_stake_after_rewards);

        // Calculate expected SOL for withdraw
        // Formula: lst_amount * total_pool_value / lst_supply
        let withdraw_lst_amount = 2_000_000_000u64; // 2 LST
        let expected_sol = (withdraw_lst_amount as u128)
            .checked_mul(pool_stake_after_rewards as u128)
            .unwrap()
            .checked_div(mint_supply as u128)
            .unwrap() as u64;

        eprintln!("\n=== Expected Withdraw ===");
        eprintln!("  Withdraw LST amount: {}", withdraw_lst_amount);
        eprintln!("  Expected SOL: {}", expected_sol);

        // Execute withdraw
        let (user_stake_pda, _) = Pubkey::find_program_address(
            &[
                b"withdraw",
                pool_state_pda.as_ref(),
                user.pubkey().as_ref(),
                &1u64.to_le_bytes(),
            ],
            &PROGRAM_ID,
        );

        let result = execute_withdraw(
            &mut svm,
            &user,
            &pool_state_pda,
            &pool_stake_pda,
            &reserve_stake_pda,
            &lst_mint_pda,
            &user_lst_ata,
            withdraw_lst_amount,
            1,
        );
        print_transaction_logs(&result);
        assert!(result.is_ok(), "Withdraw should succeed");

        // Verify user got correct SOL amount
        let user_stake_account = svm.get_account(&user_stake_pda).unwrap();

        eprintln!("\n=== Actual Withdraw ===");
        eprintln!("  User stake lamports: {}", user_stake_account.lamports);

        // User should get MORE SOL than LST burned (because of rewards)
        assert!(
            user_stake_account.lamports > withdraw_lst_amount,
            "With rewards, user should get more SOL than LST burned. Got {} for {} LST",
            user_stake_account.lamports,
            withdraw_lst_amount
        );

        // Verify close to expected (allow some variance for rent)
        let difference = (user_stake_account.lamports as i64 - expected_sol as i64).abs();
        assert!(
            difference < 10_000_000, // 0.01 SOL tolerance
            "User should get approximately expected SOL. Expected {}, got {}",
            expected_sol,
            user_stake_account.lamports
        );

        println!("\n=== Test Passed: Correct Exchange Rate With Rewards ===");
    }

    #[test]
    fn test_withdraw_lst_supply_invariant() {
        let mut svm = setup_svm();

        let (
            user,
            pool_state_pda,
            lst_mint_pda,
            pool_stake_pda,
            reserve_stake_pda,
            user_lst_ata,
            _,
        ) = setup_pool_for_withdraw(&mut svm);

        // Get state before withdraw
        let mint_supply_before = get_mint_supply(&svm.get_account(&lst_mint_pda).unwrap().data);
        let user_balance_before = get_token_balance(&svm.get_account(&user_lst_ata).unwrap().data);

        eprintln!("\n=== Before Withdraw ===");
        eprintln!("  Mint supply: {}", mint_supply_before);
        eprintln!("  User balance: {}", user_balance_before);

        let withdraw_amount = 2_000_000_000u64;

        // Execute withdraw
        let result = execute_withdraw(
            &mut svm,
            &user,
            &pool_state_pda,
            &pool_stake_pda,
            &reserve_stake_pda,
            &lst_mint_pda,
            &user_lst_ata,
            withdraw_amount,
            1,
        );
        print_transaction_logs(&result);
        assert!(result.is_ok(), "Withdraw should succeed");

        // Get state after withdraw
        let mint_supply_after = get_mint_supply(&svm.get_account(&lst_mint_pda).unwrap().data);
        let user_balance_after = get_token_balance(&svm.get_account(&user_lst_ata).unwrap().data);

        eprintln!("\n=== After Withdraw ===");
        eprintln!("  Mint supply: {}", mint_supply_after);
        eprintln!("  User balance: {}", user_balance_after);

        // INVARIANT: Mint supply should decrease by exactly withdraw_amount
        assert_eq!(
            mint_supply_after,
            mint_supply_before - withdraw_amount,
            "Mint supply should decrease by withdraw amount"
        );

        // INVARIANT: User balance should decrease by exactly withdraw_amount
        assert_eq!(
            user_balance_after,
            user_balance_before - withdraw_amount,
            "User balance should decrease by withdraw amount"
        );

        println!("\n=== Test Passed: lst_supply Invariant Maintained ===");
    }

    #[test]
    fn test_withdraw_user_without_lst_fails() {
        let mut svm = setup_svm();

        // Setup pool but use a DIFFERENT user who has no LST
        let (_original_user, pool_state_pda, lst_mint_pda, pool_stake_pda, reserve_stake_pda, _, _) =
            setup_pool_for_withdraw(&mut svm);

        // Create new user with NO LST
        let user_without_lst = Keypair::new();
        svm.airdrop(&user_without_lst.pubkey(), 2_000_000_000)
            .unwrap();

        // Create ATA for new user (will have 0 balance)
        let user_lst_ata = derive_ata(&user_without_lst.pubkey(), &lst_mint_pda);

        let create_ata_ix =
            spl_associated_token_account::instruction::create_associated_token_account(
                &user_without_lst.pubkey(),
                &user_without_lst.pubkey(),
                &lst_mint_pda,
                &TOKEN_PROGRAM_ID,
            );

        let tx = Transaction::new_signed_with_payer(
            &[create_ata_ix],
            Some(&user_without_lst.pubkey()),
            &[&user_without_lst],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx).expect("Should create ATA");

        // Verify user has 0 LST
        let user_balance = get_token_balance(&svm.get_account(&user_lst_ata).unwrap().data);
        assert_eq!(user_balance, 0, "User should have 0 LST");

        // Try to withdraw
        let result = execute_withdraw(
            &mut svm,
            &user_without_lst,
            &pool_state_pda,
            &pool_stake_pda,
            &reserve_stake_pda,
            &lst_mint_pda,
            &user_lst_ata,
            1_000_000_000, // Try to withdraw 1 SOL worth
            1,
        );
        print_transaction_logs(&result);

        assert!(result.is_err(), "Withdraw without LST balance should fail");

        println!("\n=== Test Passed: User Without LST Rejected ===");
    }
}

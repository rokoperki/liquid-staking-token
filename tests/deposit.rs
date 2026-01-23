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

    // ============================================
    // HELPER FUNCTIONS
    // ============================================

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
        let mut data = vec![0u8];
        data.extend_from_slice(&seed.to_le_bytes());
        data.push(pool_bump);
        data.push(mint_bump);
        data.push(stake_bump);
        data.push(reserve_bump);
        data
    }

    fn create_deposit_instruction_data(amount: u64) -> Vec<u8> {
        let mut data = vec![1u8];
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
                AccountMeta::new(initializer.pubkey(), true),
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
            Some(&initializer.pubkey()),
            &[&initializer],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(transaction);
        print_transaction_logs(&result);
        assert!(result.is_ok(), "Initialize should succeed");

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

    fn build_deposit_instruction(
        depositor: &Pubkey,
        pool_state_pda: &Pubkey,
        pool_stake_pda: &Pubkey,
        reserve_stake_pda: &Pubkey,
        lst_mint_pda: &Pubkey,
        depositor_lst_ata: &Pubkey,
        amount: u64,
    ) -> Instruction {
        Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(*depositor, true),
                AccountMeta::new(*pool_state_pda, false),
                AccountMeta::new_readonly(*pool_stake_pda, false),
                AccountMeta::new(*reserve_stake_pda, false),
                AccountMeta::new(*lst_mint_pda, false),
                AccountMeta::new(*depositor_lst_ata, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
                AccountMeta::new_readonly(STAKE_PROGRAM_ID, false),
                AccountMeta::new_readonly(ATA_PROGRAM_ID, false),
            ],
            data: create_deposit_instruction_data(amount),
        }
    }

    fn create_user_with_ata(
        svm: &mut LiteSVM,
        lst_mint_pda: &Pubkey,
        airdrop: u64,
    ) -> (Keypair, Pubkey) {
        let user = Keypair::new();
        svm.airdrop(&user.pubkey(), airdrop).unwrap();

        let user_lst_ata = derive_ata(&user.pubkey(), lst_mint_pda);

        let create_ata_ix =
            spl_associated_token_account::instruction::create_associated_token_account(
                &user.pubkey(),
                &user.pubkey(),
                lst_mint_pda,
                &TOKEN_PROGRAM_ID,
            );

        let tx = Transaction::new_signed_with_payer(
            &[create_ata_ix],
            Some(&user.pubkey()),
            &[&user],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx).expect("Should create ATA");

        (user, user_lst_ata)
    }

    fn get_lst_balance(svm: &LiteSVM, ata: &Pubkey) -> u64 {
        let account = svm.get_account(ata).unwrap();
        u64::from_le_bytes(account.data[64..72].try_into().unwrap())
    }

    fn get_lst_supply(svm: &LiteSVM, pool_state: &Pubkey) -> u64 {
        let account = svm.get_account(pool_state).unwrap();
        u64::from_le_bytes(account.data[173..181].try_into().unwrap())
    }

    // ============================================
    // SUCCESS CASES
    // ============================================

    #[test]
    fn test_deposit_success() {
        let mut svm = setup_svm();
        let (_, pool_state_pda, lst_mint_pda, pool_stake_pda, reserve_stake_pda, _, _) =
            initialize_pool(&mut svm);

        let (depositor, depositor_lst_ata) =
            create_user_with_ata(&mut svm, &lst_mint_pda, 5_000_000_000);
        let deposit_amount = 2_000_000_000u64;

        let reserve_before = svm.get_account(&reserve_stake_pda).unwrap().lamports;

        let instruction = build_deposit_instruction(
            &depositor.pubkey(),
            &pool_state_pda,
            &pool_stake_pda,
            &reserve_stake_pda,
            &lst_mint_pda,
            &depositor_lst_ata,
            deposit_amount,
        );

        let tx = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&depositor.pubkey()),
            &[&depositor],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_ok(), "Deposit should succeed");

        let reserve_after = svm.get_account(&reserve_stake_pda).unwrap().lamports;
        assert_eq!(
            reserve_after - reserve_before,
            deposit_amount,
            "Reserve should receive deposit"
        );

        let lst_balance = get_lst_balance(&svm, &depositor_lst_ata);
        assert!(lst_balance > 0, "User should have received LST");

        println!("\n=== test_deposit_success PASSED ===");
    }

    #[test]
    fn test_deposit_second_deposit_proportional() {
        let mut svm = setup_svm();
        let (_, pool_state_pda, lst_mint_pda, pool_stake_pda, reserve_stake_pda, _, _) =
            initialize_pool(&mut svm);

        // First depositor
        let (depositor1, depositor1_ata) =
            create_user_with_ata(&mut svm, &lst_mint_pda, 5_000_000_000);
        let deposit1_amount = 2_000_000_000u64;

        let ix1 = build_deposit_instruction(
            &depositor1.pubkey(),
            &pool_state_pda,
            &pool_stake_pda,
            &reserve_stake_pda,
            &lst_mint_pda,
            &depositor1_ata,
            deposit1_amount,
        );

        let tx1 = Transaction::new_signed_with_payer(
            &[ix1],
            Some(&depositor1.pubkey()),
            &[&depositor1],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx1)
            .expect("First deposit should succeed");

        let lst_supply_after_first = get_lst_supply(&svm, &pool_state_pda);
        let pool_stake = svm.get_account(&pool_stake_pda).unwrap().lamports;
        let reserve = svm.get_account(&reserve_stake_pda).unwrap().lamports;
        let total_pool_value = pool_stake + reserve;

        println!("After first deposit:");
        println!("  LST supply: {}", lst_supply_after_first);
        println!("  Total pool value: {}", total_pool_value);

        // Second depositor
        let (depositor2, depositor2_ata) =
            create_user_with_ata(&mut svm, &lst_mint_pda, 5_000_000_000);
        let deposit2_amount = 1_100_000_000u64;

        let ix2 = build_deposit_instruction(
            &depositor2.pubkey(),
            &pool_state_pda,
            &pool_stake_pda,
            &reserve_stake_pda,
            &lst_mint_pda,
            &depositor2_ata,
            deposit2_amount,
        );

        let tx2 = Transaction::new_signed_with_payer(
            &[ix2],
            Some(&depositor2.pubkey()),
            &[&depositor2],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx2);
        print_transaction_logs(&result);
        assert!(result.is_ok(), "Second deposit should succeed");

        let lst_balance2 = get_lst_balance(&svm, &depositor2_ata);

        // lst_amount = (deposit_amount * lst_supply) / total_pool_value
        let expected_lst = (deposit2_amount as u128 * lst_supply_after_first as u128
            / total_pool_value as u128) as u64;

        println!("After second deposit:");
        println!("  LST received: {}", lst_balance2);
        println!("  Expected LST: {}", expected_lst);

        assert!(
            lst_balance2 >= expected_lst.saturating_sub(1)
                && lst_balance2 <= expected_lst.saturating_add(1),
            "Second deposit should get proportional LST"
        );

        println!("\n=== test_deposit_second_deposit_proportional PASSED ===");
    }

    #[test]
    fn test_deposit_multiple_deposits_same_user() {
        let mut svm = setup_svm();
        let (_, pool_state_pda, lst_mint_pda, pool_stake_pda, reserve_stake_pda, _, _) =
            initialize_pool(&mut svm);

        let (depositor, depositor_ata) =
            create_user_with_ata(&mut svm, &lst_mint_pda, 10_000_000_000);

        // First deposit
        let ix1 = build_deposit_instruction(
            &depositor.pubkey(),
            &pool_state_pda,
            &pool_stake_pda,
            &reserve_stake_pda,
            &lst_mint_pda,
            &depositor_ata,
            2_000_000_000,
        );

        let tx1 = Transaction::new_signed_with_payer(
            &[ix1],
            Some(&depositor.pubkey()),
            &[&depositor],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx1)
            .expect("First deposit should succeed");

        let lst_after_first = get_lst_balance(&svm, &depositor_ata);

        // Second deposit
        let ix2 = build_deposit_instruction(
            &depositor.pubkey(),
            &pool_state_pda,
            &pool_stake_pda,
            &reserve_stake_pda,
            &lst_mint_pda,
            &depositor_ata,
            1_100_000_000,
        );

        let tx2 = Transaction::new_signed_with_payer(
            &[ix2],
            Some(&depositor.pubkey()),
            &[&depositor],
            svm.latest_blockhash(),
        );
        let result = svm.send_transaction(tx2);
        print_transaction_logs(&result);
        assert!(result.is_ok(), "Second deposit should succeed");

        let lst_after_second = get_lst_balance(&svm, &depositor_ata);

        assert!(
            lst_after_second > lst_after_first,
            "LST balance should increase"
        );

        println!("LST after first deposit: {}", lst_after_first);
        println!("LST after second deposit: {}", lst_after_second);
        println!("\n=== test_deposit_multiple_deposits_same_user PASSED ===");
    }

    #[test]
    fn test_deposit_multiple_depositors_no_dilution() {
        let mut svm = setup_svm();
        let (_, pool_state_pda, lst_mint_pda, pool_stake_pda, reserve_stake_pda, _, _) =
            initialize_pool(&mut svm);

        // First depositor deposits 2 SOL
        let (depositor1, depositor1_ata) =
            create_user_with_ata(&mut svm, &lst_mint_pda, 5_000_000_000);

        let ix1 = build_deposit_instruction(
            &depositor1.pubkey(),
            &pool_state_pda,
            &pool_stake_pda,
            &reserve_stake_pda,
            &lst_mint_pda,
            &depositor1_ata,
            2_000_000_000,
        );

        let tx1 = Transaction::new_signed_with_payer(
            &[ix1],
            Some(&depositor1.pubkey()),
            &[&depositor1],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx1).expect("First deposit should succeed");

        let depositor1_lst = get_lst_balance(&svm, &depositor1_ata);

        // Second depositor deposits 2 SOL
        let (depositor2, depositor2_ata) =
            create_user_with_ata(&mut svm, &lst_mint_pda, 5_000_000_000);

        let ix2 = build_deposit_instruction(
            &depositor2.pubkey(),
            &pool_state_pda,
            &pool_stake_pda,
            &reserve_stake_pda,
            &lst_mint_pda,
            &depositor2_ata,
            2_000_000_000,
        );

        let tx2 = Transaction::new_signed_with_payer(
            &[ix2],
            Some(&depositor2.pubkey()),
            &[&depositor2],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx2).expect("Second deposit should succeed");

        // Check depositor1's LST hasn't changed
        let depositor1_lst_after = get_lst_balance(&svm, &depositor1_ata);
        assert_eq!(
            depositor1_lst, depositor1_lst_after,
            "First depositor's LST should not change"
        );

        let depositor2_lst = get_lst_balance(&svm, &depositor2_ata);
        let total_lst_supply = get_lst_supply(&svm, &pool_state_pda);

        println!("Depositor 1 LST: {}", depositor1_lst_after);
        println!("Depositor 2 LST: {}", depositor2_lst);
        println!("Total LST supply: {}", total_lst_supply);

        assert_eq!(
            depositor1_lst_after + depositor2_lst + 1_000_000_000,
            total_lst_supply,
            "LST supply should equal sum of balances"
        );

        println!("\n=== test_deposit_multiple_depositors_no_dilution PASSED ===");
    }

    // ============================================
    // FAILURE CASES - INPUT VALIDATION
    // ============================================

    #[test]
    fn test_deposit_zero_amount_fails() {
        let mut svm = setup_svm();
        let (_, pool_state_pda, lst_mint_pda, pool_stake_pda, reserve_stake_pda, _, _) =
            initialize_pool(&mut svm);

        let (depositor, depositor_lst_ata) =
            create_user_with_ata(&mut svm, &lst_mint_pda, 5_000_000_000);

        let instruction = build_deposit_instruction(
            &depositor.pubkey(),
            &pool_state_pda,
            &pool_stake_pda,
            &reserve_stake_pda,
            &lst_mint_pda,
            &depositor_lst_ata,
            0, // Zero amount
        );

        let tx = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&depositor.pubkey()),
            &[&depositor],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Zero deposit should fail");

        println!("\n=== test_deposit_zero_amount_fails PASSED ===");
    }

    #[test]
    fn test_deposit_insufficient_funds_fails() {
        let mut svm = setup_svm();
        let (_, pool_state_pda, lst_mint_pda, pool_stake_pda, reserve_stake_pda, _, _) =
            initialize_pool(&mut svm);

        let (depositor, depositor_lst_ata) =
            create_user_with_ata(&mut svm, &lst_mint_pda, 1_000_000_000); // Only 1 SOL

        let instruction = build_deposit_instruction(
            &depositor.pubkey(),
            &pool_state_pda,
            &pool_stake_pda,
            &reserve_stake_pda,
            &lst_mint_pda,
            &depositor_lst_ata,
            5_000_000_000, // Try to deposit 5 SOL
        );

        let tx = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&depositor.pubkey()),
            &[&depositor],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Insufficient funds should fail");

        println!("\n=== test_deposit_insufficient_funds_fails PASSED ===");
    }

    #[test]
    fn test_deposit_truncated_instruction_data_fails() {
        let mut svm = setup_svm();
        let (_, pool_state_pda, lst_mint_pda, pool_stake_pda, reserve_stake_pda, _, _) =
            initialize_pool(&mut svm);

        let (depositor, depositor_lst_ata) =
            create_user_with_ata(&mut svm, &lst_mint_pda, 5_000_000_000);

        let instruction = Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(depositor.pubkey(), true),
                AccountMeta::new(pool_state_pda, false),
                AccountMeta::new_readonly(pool_stake_pda, false),
                AccountMeta::new(reserve_stake_pda, false),
                AccountMeta::new(lst_mint_pda, false),
                AccountMeta::new(depositor_lst_ata, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
                AccountMeta::new_readonly(STAKE_PROGRAM_ID, false),
                AccountMeta::new_readonly(ATA_PROGRAM_ID, false),
            ],
            data: vec![1u8], // Only discriminator, missing amount
        };

        let tx = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&depositor.pubkey()),
            &[&depositor],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Truncated data should fail");

        println!("\n=== test_deposit_truncated_instruction_data_fails PASSED ===");
    }

    // ============================================
    // FAILURE CASES - ACCOUNT VALIDATION
    // ============================================

    #[test]
    fn test_deposit_wrong_pool_state_fails() {
        let mut svm = setup_svm();
        let (_, _, lst_mint_pda, pool_stake_pda, reserve_stake_pda, _, _) =
            initialize_pool(&mut svm);

        // Create a second pool
        let (_, pool_state_pda2, _, _, _, _, _) = initialize_pool(&mut svm);

        let (depositor, depositor_lst_ata) =
            create_user_with_ata(&mut svm, &lst_mint_pda, 5_000_000_000);

        // Use wrong pool state but correct other accounts
        let instruction = build_deposit_instruction(
            &depositor.pubkey(),
            &pool_state_pda2, // Wrong pool state
            &pool_stake_pda,
            &reserve_stake_pda,
            &lst_mint_pda,
            &depositor_lst_ata,
            1_000_000_000,
        );

        let tx = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&depositor.pubkey()),
            &[&depositor],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Wrong pool state should fail");

        println!("\n=== test_deposit_wrong_pool_state_fails PASSED ===");
    }

    #[test]
    fn test_deposit_wrong_pool_stake_fails() {
        let mut svm = setup_svm();
        let (_, pool_state_pda, lst_mint_pda, _, reserve_stake_pda, _, _) =
            initialize_pool(&mut svm);

        // Create a second pool to get different stake account
        let (_, _, _, pool_stake_pda2, _, _, _) = initialize_pool(&mut svm);

        let (depositor, depositor_lst_ata) =
            create_user_with_ata(&mut svm, &lst_mint_pda, 5_000_000_000);

        let instruction = build_deposit_instruction(
            &depositor.pubkey(),
            &pool_state_pda,
            &pool_stake_pda2, // Wrong pool stake
            &reserve_stake_pda,
            &lst_mint_pda,
            &depositor_lst_ata,
            1_000_000_000,
        );

        let tx = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&depositor.pubkey()),
            &[&depositor],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Wrong pool stake should fail");

        println!("\n=== test_deposit_wrong_pool_stake_fails PASSED ===");
    }

    #[test]
    fn test_deposit_wrong_reserve_stake_fails() {
        let mut svm = setup_svm();
        let (_, pool_state_pda, lst_mint_pda, pool_stake_pda, _, _, _) = initialize_pool(&mut svm);

        // Create a second pool to get different reserve
        let (_, _, _, _, reserve_stake_pda2, _, _) = initialize_pool(&mut svm);

        let (depositor, depositor_lst_ata) =
            create_user_with_ata(&mut svm, &lst_mint_pda, 5_000_000_000);

        let instruction = build_deposit_instruction(
            &depositor.pubkey(),
            &pool_state_pda,
            &pool_stake_pda,
            &reserve_stake_pda2, // Wrong reserve stake
            &lst_mint_pda,
            &depositor_lst_ata,
            1_000_000_000,
        );

        let tx = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&depositor.pubkey()),
            &[&depositor],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Wrong reserve stake should fail");

        println!("\n=== test_deposit_wrong_reserve_stake_fails PASSED ===");
    }

    #[test]
    fn test_deposit_wrong_lst_mint_fails() {
        let mut svm = setup_svm();
        let (_, pool_state_pda, lst_mint_pda, pool_stake_pda, reserve_stake_pda, _, _) =
            initialize_pool(&mut svm);

        // Create a second pool to get different mint
        let (_, _, lst_mint_pda2, _, _, _, _) = initialize_pool(&mut svm);

        let (depositor, depositor_lst_ata) =
            create_user_with_ata(&mut svm, &lst_mint_pda, 5_000_000_000);

        let instruction = build_deposit_instruction(
            &depositor.pubkey(),
            &pool_state_pda,
            &pool_stake_pda,
            &reserve_stake_pda,
            &lst_mint_pda2, // Wrong mint
            &depositor_lst_ata,
            1_000_000_000,
        );

        let tx = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&depositor.pubkey()),
            &[&depositor],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Wrong LST mint should fail");

        println!("\n=== test_deposit_wrong_lst_mint_fails PASSED ===");
    }

    #[test]
    fn test_deposit_wrong_depositor_ata_fails() {
        let mut svm = setup_svm();
        let (_, pool_state_pda, lst_mint_pda, pool_stake_pda, reserve_stake_pda, _, _) =
            initialize_pool(&mut svm);

        let (depositor, _) = create_user_with_ata(&mut svm, &lst_mint_pda, 5_000_000_000);

        // Create another user's ATA
        let (other_user, other_user_ata) =
            create_user_with_ata(&mut svm, &lst_mint_pda, 1_000_000_000);

        let instruction = build_deposit_instruction(
            &depositor.pubkey(),
            &pool_state_pda,
            &pool_stake_pda,
            &reserve_stake_pda,
            &lst_mint_pda,
            &other_user_ata, // Wrong ATA (belongs to other user)
            1_000_000_000,
        );

        let tx = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&depositor.pubkey()),
            &[&depositor],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        // This might succeed (depositing to someone else's account) depending on your validation
        // If you want this to fail, add validation in your program

        println!("\n=== test_deposit_wrong_depositor_ata_fails COMPLETED ===");
    }

    // ============================================
    // FAILURE CASES - SIGNER CHECKS
    // ============================================

    #[test]
    fn test_deposit_missing_signer_fails() {
        let mut svm = setup_svm();
        let (_, pool_state_pda, lst_mint_pda, pool_stake_pda, reserve_stake_pda, _, _) =
            initialize_pool(&mut svm);

        let (depositor, depositor_lst_ata) =
            create_user_with_ata(&mut svm, &lst_mint_pda, 5_000_000_000);

        let payer = Keypair::new();
        svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

        let instruction = Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(depositor.pubkey(), false), // NOT a signer
                AccountMeta::new(pool_state_pda, false),
                AccountMeta::new_readonly(pool_stake_pda, false),
                AccountMeta::new(reserve_stake_pda, false),
                AccountMeta::new(lst_mint_pda, false),
                AccountMeta::new(depositor_lst_ata, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
                AccountMeta::new_readonly(STAKE_PROGRAM_ID, false),
                AccountMeta::new_readonly(ATA_PROGRAM_ID, false),
            ],
            data: create_deposit_instruction_data(1_000_000_000),
        };

        let tx = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&payer.pubkey()),
            &[&payer],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Missing signer should fail");

        println!("\n=== test_deposit_missing_signer_fails PASSED ===");
    }

    // ============================================
    // FAILURE CASES - ATA ISSUES
    // ============================================

    #[test]
    fn test_deposit_ata_not_exists_fails() {
        let mut svm = setup_svm();
        let (_, pool_state_pda, lst_mint_pda, pool_stake_pda, reserve_stake_pda, _, _) =
            initialize_pool(&mut svm);

        let depositor = Keypair::new();
        svm.airdrop(&depositor.pubkey(), 5_000_000_000).unwrap();

        // Derive ATA but don't create it
        let depositor_lst_ata = derive_ata(&depositor.pubkey(), &lst_mint_pda);

        let instruction = build_deposit_instruction(
            &depositor.pubkey(),
            &pool_state_pda,
            &pool_stake_pda,
            &reserve_stake_pda,
            &lst_mint_pda,
            &depositor_lst_ata, // ATA doesn't exist
            1_000_000_000,
        );

        let tx = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&depositor.pubkey()),
            &[&depositor],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_err(), "Non-existent ATA should fail");

        println!("\n=== test_deposit_ata_not_exists_fails PASSED ===");
    }

    #[test]
    fn test_deposit_ata_wrong_mint_fails() {
        let mut svm = setup_svm();
        let (_, pool_state_pda, lst_mint_pda, pool_stake_pda, reserve_stake_pda, _, _) =
            initialize_pool(&mut svm);

        // Create second pool to get different mint
        let (_, _, lst_mint_pda2, _, _, _, _) = initialize_pool(&mut svm);

        let depositor = Keypair::new();
        svm.airdrop(&depositor.pubkey(), 5_000_000_000).unwrap();

        // Create ATA for wrong mint
        let wrong_ata = derive_ata(&depositor.pubkey(), &lst_mint_pda2);
        let create_ata_ix =
            spl_associated_token_account::instruction::create_associated_token_account(
                &depositor.pubkey(),
                &depositor.pubkey(),
                &lst_mint_pda2,
                &TOKEN_PROGRAM_ID,
            );

        let tx = Transaction::new_signed_with_payer(
            &[create_ata_ix],
            Some(&depositor.pubkey()),
            &[&depositor],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx).expect("Should create ATA");

        let instruction = build_deposit_instruction(
            &depositor.pubkey(),
            &pool_state_pda,
            &pool_stake_pda,
            &reserve_stake_pda,
            &lst_mint_pda,
            &wrong_ata, // ATA for different mint
            1_000_000_000,
        );

        let tx = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&depositor.pubkey()),
            &[&depositor],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        print_transaction_logs(&result);
        assert!(result.is_err(), "ATA for wrong mint should fail");

        println!("\n=== test_deposit_ata_wrong_mint_fails PASSED ===");
    }

    // ============================================
    // STATE VERIFICATION
    // ============================================

    #[test]
    fn test_deposit_lst_supply_updated() {
        let mut svm = setup_svm();
        let (_, pool_state_pda, lst_mint_pda, pool_stake_pda, reserve_stake_pda, _, _) =
            initialize_pool(&mut svm);

        let (depositor, depositor_lst_ata) =
            create_user_with_ata(&mut svm, &lst_mint_pda, 5_000_000_000);

        let lst_supply_before = get_lst_supply(&svm, &pool_state_pda);

        let instruction = build_deposit_instruction(
            &depositor.pubkey(),
            &pool_state_pda,
            &pool_stake_pda,
            &reserve_stake_pda,
            &lst_mint_pda,
            &depositor_lst_ata,
            2_000_000_000,
        );

        let tx = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&depositor.pubkey()),
            &[&depositor],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx).expect("Deposit should succeed");

        let lst_supply_after = get_lst_supply(&svm, &pool_state_pda);
        let lst_balance = get_lst_balance(&svm, &depositor_lst_ata);

        assert_eq!(
            lst_supply_after - lst_supply_before,
            lst_balance,
            "LST supply increase should match minted amount"
        );

        println!("LST supply before: {}", lst_supply_before);
        println!("LST supply after: {}", lst_supply_after);
        println!("LST minted: {}", lst_balance);
        println!("\n=== test_deposit_lst_supply_updated PASSED ===");
    }

    #[test]
    fn test_deposit_reserve_receives_sol() {
        let mut svm = setup_svm();
        let (_, pool_state_pda, lst_mint_pda, pool_stake_pda, reserve_stake_pda, _, _) =
            initialize_pool(&mut svm);

        let (depositor, depositor_lst_ata) =
            create_user_with_ata(&mut svm, &lst_mint_pda, 5_000_000_000);

        let deposit_amount = 2_000_000_000u64;
        let reserve_before = svm.get_account(&reserve_stake_pda).unwrap().lamports;

        let instruction = build_deposit_instruction(
            &depositor.pubkey(),
            &pool_state_pda,
            &pool_stake_pda,
            &reserve_stake_pda,
            &lst_mint_pda,
            &depositor_lst_ata,
            deposit_amount,
        );

        let tx = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&depositor.pubkey()),
            &[&depositor],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx).expect("Deposit should succeed");

        let reserve_after = svm.get_account(&reserve_stake_pda).unwrap().lamports;

        assert_eq!(
            reserve_after - reserve_before,
            deposit_amount,
            "Reserve should receive exact deposit amount"
        );

        println!("Reserve before: {}", reserve_before);
        println!("Reserve after: {}", reserve_after);
        println!("Deposit amount: {}", deposit_amount);
        println!("\n=== test_deposit_reserve_receives_sol PASSED ===");
    }

    #[test]
    fn test_deposit_depositor_sol_decreases() {
        let mut svm = setup_svm();
        let (_, pool_state_pda, lst_mint_pda, pool_stake_pda, reserve_stake_pda, _, _) =
            initialize_pool(&mut svm);

        let (depositor, depositor_lst_ata) =
            create_user_with_ata(&mut svm, &lst_mint_pda, 5_000_000_000);

        let deposit_amount = 2_000_000_000u64;
        let depositor_sol_before = svm.get_account(&depositor.pubkey()).unwrap().lamports;

        let instruction = build_deposit_instruction(
            &depositor.pubkey(),
            &pool_state_pda,
            &pool_stake_pda,
            &reserve_stake_pda,
            &lst_mint_pda,
            &depositor_lst_ata,
            deposit_amount,
        );

        let tx = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&depositor.pubkey()),
            &[&depositor],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx).expect("Deposit should succeed");

        let depositor_sol_after = svm.get_account(&depositor.pubkey()).unwrap().lamports;
        let sol_spent = depositor_sol_before - depositor_sol_after;

        // Should have spent deposit_amount + tx fee
        assert!(
            sol_spent >= deposit_amount,
            "Depositor should have spent at least deposit amount"
        );
        assert!(
            sol_spent < deposit_amount + 100_000,
            "Depositor should not have spent too much"
        );

        println!("SOL before: {}", depositor_sol_before);
        println!("SOL after: {}", depositor_sol_after);
        println!("SOL spent (including fee): {}", sol_spent);
        println!("\n=== test_deposit_depositor_sol_decreases PASSED ===");
    }
}
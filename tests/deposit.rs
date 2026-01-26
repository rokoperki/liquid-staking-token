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
    fn test_deposit_success() {
        let mut svm = setup_svm();

        // Initialize pool first
        let (_, pool_state_pda, lst_mint_pda, pool_stake_pda, reserve_stake_pda, validator_vote, _) =
            initialize_pool(&mut svm);

        // Verify initialization worked
        let pool_stake_account = svm.get_account(&pool_stake_pda);
        assert!(pool_stake_account.is_some(), "Pool stake should exist");

        let reserve_stake_account = svm.get_account(&reserve_stake_pda);
        assert!(
            reserve_stake_account.is_some(),
            "Reserve stake should exist"
        );

        // Create depositor
        let depositor = Keypair::new();
        let deposit_amount = 1_200_000_000u64;
        svm.airdrop(&depositor.pubkey(), 2_000_000_000).unwrap();

        // Derive and CREATE depositor's LST ATA
        let depositor_lst_ata = derive_ata(&depositor.pubkey(), &lst_mint_pda);

        let create_ata_ix =
            spl_associated_token_account::instruction::create_associated_token_account(
                &depositor.pubkey(),
                &depositor.pubkey(),
                &lst_mint_pda,
                &TOKEN_PROGRAM_ID,
            );

        let tx = Transaction::new_signed_with_payer(
            &[create_ata_ix],
            Some(&depositor.pubkey()),
            &[&depositor],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx).expect("Should create ATA");

        let instruction_data = create_deposit_instruction_data(deposit_amount);

        let instruction = Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(depositor.pubkey(), true),
                AccountMeta::new(pool_state_pda, false),
                AccountMeta::new_readonly(pool_stake_pda, false),
                AccountMeta::new(reserve_stake_pda, false), // ✅ Changed to writable
                AccountMeta::new(lst_mint_pda, false),
                AccountMeta::new(depositor_lst_ata, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
                AccountMeta::new_readonly(STAKE_PROGRAM_ID, false),
                AccountMeta::new_readonly(ATA_PROGRAM_ID, false),
            ],
            data: instruction_data,
        };

        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&depositor.pubkey()),
            &[&depositor],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(transaction);
        print_transaction_logs(&result);
        assert!(result.is_ok(), "Deposit should succeed");

        // ✅ Verify reserve stake received SOL
        let reserve_after = svm.get_account(&reserve_stake_pda).unwrap();
        assert!(
            reserve_after.lamports >= deposit_amount,
            "Reserve should have received deposit"
        );

        println!(
            "Reserve stake lamports before deposit: {}",
            reserve_after.lamports - deposit_amount
        );
        println!(
            "Reserve stake lamports after deposit: {}",
            reserve_after.lamports
        );
        println!("\n=== All Verifications Passed ===");
    }

    /// Helper to get token account balance from account data
    fn get_token_balance(account_data: &[u8]) -> u64 {
        u64::from_le_bytes(account_data[64..72].try_into().unwrap())
    }

    /// Helper to get mint total supply
    fn get_mint_supply(mint_data: &[u8]) -> u64 {
        u64::from_le_bytes(mint_data[36..44].try_into().unwrap())
    }

    /// Helper to create depositor ATA and return the address
    fn create_depositor_ata(svm: &mut LiteSVM, depositor: &Keypair, lst_mint: &Pubkey) -> Pubkey {
        let depositor_lst_ata = derive_ata(&depositor.pubkey(), lst_mint);

        let create_ata_ix =
            spl_associated_token_account::instruction::create_associated_token_account(
                &depositor.pubkey(),
                &depositor.pubkey(),
                lst_mint,
                &TOKEN_PROGRAM_ID,
            );

        let tx = Transaction::new_signed_with_payer(
            &[create_ata_ix],
            Some(&depositor.pubkey()),
            &[&depositor],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx).expect("Should create ATA");

        depositor_lst_ata
    }

    /// Helper to execute deposit
    fn execute_deposit(
        svm: &mut LiteSVM,
        depositor: &Keypair,
        pool_state_pda: &Pubkey,
        pool_stake_pda: &Pubkey,
        reserve_stake_pda: &Pubkey,
        lst_mint_pda: &Pubkey,
        depositor_lst_ata: &Pubkey,
        amount: u64,
    ) -> Result<litesvm::types::TransactionMetadata, litesvm::types::FailedTransactionMetadata>
    {
        let instruction_data = create_deposit_instruction_data(amount);

        let instruction = Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(depositor.pubkey(), true),
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
            data: instruction_data,
        };

        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&depositor.pubkey()),
            &[&depositor],
            svm.latest_blockhash(),
        );

        svm.send_transaction(transaction)
    }

    const MIN_STAKE_DELEGATION: u64 = 1_000_000_000;

    #[test]
    fn test_first_deposit_gets_1_to_1_rate() {
        let mut svm = setup_svm();

        // Initialize pool
        let (initializer, pool_state_pda, lst_mint_pda, pool_stake_pda, reserve_stake_pda, _, _) =
            initialize_pool(&mut svm);

        // Get state after initialize
        let pool_stake_lamports = svm.get_account(&pool_stake_pda).unwrap().lamports;
        let reserve_lamports = svm.get_account(&reserve_stake_pda).unwrap().lamports;
        let total_pool_value_before = pool_stake_lamports + reserve_lamports;

        let mint_supply_before = get_mint_supply(&svm.get_account(&lst_mint_pda).unwrap().data);

        // Verify initializer got their LST
        let initializer_lst_ata =
            get_associated_token_address(&initializer.pubkey(), &lst_mint_pda);
        let initializer_balance =
            get_token_balance(&svm.get_account(&initializer_lst_ata).unwrap().data);

        eprintln!("\n=== State After Initialize ===");
        eprintln!("  Pool stake: {} lamports", pool_stake_lamports);
        eprintln!("  Reserve: {} lamports", reserve_lamports);
        eprintln!("  Total pool value: {} lamports", total_pool_value_before);
        eprintln!("  Mint supply: {}", mint_supply_before);
        eprintln!("  Initializer LST balance: {}", initializer_balance);

        // Create depositor
        let depositor = Keypair::new();
        let deposit_amount = 1_200_000_000u64; // 1 SOL
        svm.airdrop(&depositor.pubkey(), 2_000_000_000).unwrap();

        let depositor_lst_ata = create_depositor_ata(&mut svm, &depositor, &lst_mint_pda);

        // Execute deposit
        let result = execute_deposit(
            &mut svm,
            &depositor,
            &pool_state_pda,
            &pool_stake_pda,
            &reserve_stake_pda,
            &lst_mint_pda,
            &depositor_lst_ata,
            deposit_amount,
        );
        print_transaction_logs(&result);
        assert!(result.is_ok(), "Deposit should succeed");

        // Verify depositor received LST
        let depositor_balance =
            get_token_balance(&svm.get_account(&depositor_lst_ata).unwrap().data);

        // Calculate expected LST: deposit * lst_supply / total_pool_value
        // Since no rewards yet, rate should be ~1:1
        let expected_lst = (deposit_amount as u128)
            .checked_mul(mint_supply_before as u128)
            .unwrap()
            .checked_div(total_pool_value_before as u128)
            .unwrap() as u64;

        eprintln!("\n=== First Deposit Results ===");
        eprintln!("  Deposit amount: {} lamports", deposit_amount);
        eprintln!("  Expected LST: {}", expected_lst);
        eprintln!("  Actual LST received: {}", depositor_balance);

        // Allow small variance due to rent in stake accounts
        let rate = depositor_balance as f64 / deposit_amount as f64;
        eprintln!("  Effective rate: {:.6} LST per SOL", rate);

        // Rate should be very close to 1:1 (within 1% due to rent)
        assert!(
            rate > 0.99 && rate <= 1.0,
            "First deposit should get ~1:1 rate. Got {} LST for {} SOL (rate: {})",
            depositor_balance,
            deposit_amount,
            rate
        );

        // Depositor should not get MORE than deposited (would dilute initializer)
        assert!(
            depositor_balance <= deposit_amount,
            "Depositor should not get more LST than SOL deposited"
        );

        println!("\n=== Test Passed: First Deposit Gets ~1:1 Rate ===");
    }

    #[test]
    fn test_deposit_after_rewards_no_dilution() {
        let mut svm = setup_svm();

        // Initialize pool
        let (initializer, pool_state_pda, lst_mint_pda, pool_stake_pda, reserve_stake_pda, _, _) =
            initialize_pool(&mut svm);

        // Get initializer's LST balance
        let initializer_lst_ata =
            get_associated_token_address(&initializer.pubkey(), &lst_mint_pda);
        let initializer_lst_balance =
            get_token_balance(&svm.get_account(&initializer_lst_ata).unwrap().data);

        // Record state before "rewards"
        let pool_stake_before = svm.get_account(&pool_stake_pda).unwrap().lamports;
        let reserve_before = svm.get_account(&reserve_stake_pda).unwrap().lamports;
        let total_before = pool_stake_before + reserve_before;
        let mint_supply_before = get_mint_supply(&svm.get_account(&lst_mint_pda).unwrap().data);

        eprintln!("\n=== State Before Rewards ===");
        eprintln!("  Total pool value: {} lamports", total_before);
        eprintln!("  LST supply: {}", mint_supply_before);
        eprintln!(
            "  Exchange rate: {:.6} SOL per LST",
            total_before as f64 / mint_supply_before as f64
        );

        // Simulate staking rewards by adding SOL to reserve
        // In real life, this would come from staking rewards on the stake account
        let reward_amount = 500_000_000u64; // 0.5 SOL rewards
        let reserve_account = svm.get_account(&reserve_stake_pda).unwrap();
        svm.set_account(
            reserve_stake_pda,
            Account {
                lamports: reserve_account.lamports + reward_amount,
                data: reserve_account.data.clone(),
                owner: reserve_account.owner,
                executable: false,
                rent_epoch: 0,
            }
            .into(),
        );

        // Verify rewards applied
        let total_after_rewards = svm.get_account(&pool_stake_pda).unwrap().lamports
            + svm.get_account(&reserve_stake_pda).unwrap().lamports;

        let exchange_rate_after_rewards = total_after_rewards as f64 / mint_supply_before as f64;

        eprintln!("\n=== State After Rewards ===");
        eprintln!("  Total pool value: {} lamports", total_after_rewards);
        eprintln!("  LST supply: {} (unchanged)", mint_supply_before);
        eprintln!(
            "  Exchange rate: {:.6} SOL per LST",
            exchange_rate_after_rewards
        );

        // Calculate initializer's value BEFORE new deposit
        let initializer_value_before = (initializer_lst_balance as f64 / mint_supply_before as f64)
            * total_after_rewards as f64;

        eprintln!(
            "  Initializer's pool value: {:.0} lamports",
            initializer_value_before
        );

        // Now a new depositor comes in
        let depositor = Keypair::new();
        let deposit_amount = 1_200_000_000u64; // 1 SOL
        svm.airdrop(&depositor.pubkey(), 2_000_000_000).unwrap();

        let depositor_lst_ata = create_depositor_ata(&mut svm, &depositor, &lst_mint_pda);

        // Execute deposit
        let result = execute_deposit(
            &mut svm,
            &depositor,
            &pool_state_pda,
            &pool_stake_pda,
            &reserve_stake_pda,
            &lst_mint_pda,
            &depositor_lst_ata,
            deposit_amount,
        );
        print_transaction_logs(&result);
        assert!(result.is_ok(), "Deposit should succeed");

        // Get depositor's LST balance
        let depositor_lst_balance =
            get_token_balance(&svm.get_account(&depositor_lst_ata).unwrap().data);

        // Calculate expected LST (should be LESS than deposit due to rewards)
        let expected_lst = (deposit_amount as u128)
            .checked_mul(mint_supply_before as u128)
            .unwrap()
            .checked_div(total_after_rewards as u128)
            .unwrap() as u64;

        eprintln!("\n=== Deposit After Rewards Results ===");
        eprintln!("  Deposit amount: {} lamports", deposit_amount);
        eprintln!("  Expected LST: {}", expected_lst);
        eprintln!("  Actual LST received: {}", depositor_lst_balance);

        // Depositor should get LESS LST than SOL deposited (rewards make LST worth more)
        assert!(
            depositor_lst_balance < deposit_amount,
            "Depositor should get fewer LST than SOL deposited when rewards exist. Got {} LST for {} SOL",
            depositor_lst_balance,
            deposit_amount
        );

        // Verify depositor got approximately expected amount
        let tolerance = expected_lst / 100; // 1% tolerance
        assert!(
            (depositor_lst_balance as i64 - expected_lst as i64).abs() <= tolerance as i64,
            "Depositor LST should be close to expected. Expected {}, got {}",
            expected_lst,
            depositor_lst_balance
        );

        // CRITICAL: Verify initializer wasn't diluted
        let mint_supply_after = get_mint_supply(&svm.get_account(&lst_mint_pda).unwrap().data);
        let total_after_deposit = svm.get_account(&pool_stake_pda).unwrap().lamports
            + svm.get_account(&reserve_stake_pda).unwrap().lamports;

        let initializer_value_after = (initializer_lst_balance as f64 / mint_supply_after as f64)
            * total_after_deposit as f64;

        eprintln!("\n=== Dilution Check ===");
        eprintln!(
            "  Initializer value before deposit: {:.0} lamports",
            initializer_value_before
        );
        eprintln!(
            "  Initializer value after deposit: {:.0} lamports",
            initializer_value_after
        );

        // Initializer's value should NOT decrease (may increase slightly due to rounding in their favor)
        assert!(
            initializer_value_after >= initializer_value_before - 1.0, // Allow 1 lamport rounding
            "Initializer was diluted! Value went from {} to {}",
            initializer_value_before,
            initializer_value_after
        );

        println!("\n=== Test Passed: No Dilution After Rewards ===");
    }

    #[test]
    fn test_lst_supply_invariant_after_deposit() {
        let mut svm = setup_svm();

        // Initialize pool
        let (_, pool_state_pda, lst_mint_pda, pool_stake_pda, reserve_stake_pda, _, _) =
            initialize_pool(&mut svm);

        // Get mint supply before deposit
        let mint_supply_before = get_mint_supply(&svm.get_account(&lst_mint_pda).unwrap().data);

        eprintln!("\n=== State Before Deposit ===");
        eprintln!("  Mint total supply: {}", mint_supply_before);

        // Create depositor and deposit
        let depositor = Keypair::new();
        let deposit_amount = 1_500_000_000u64;
        svm.airdrop(&depositor.pubkey(), 3_000_000_000).unwrap();

        let depositor_lst_ata = create_depositor_ata(&mut svm, &depositor, &lst_mint_pda);

        // Get pool value for expected calculation
        let total_pool_before = svm.get_account(&pool_stake_pda).unwrap().lamports
            + svm.get_account(&reserve_stake_pda).unwrap().lamports;

        let expected_lst_minted = (deposit_amount as u128)
            .checked_mul(mint_supply_before as u128)
            .unwrap()
            .checked_div(total_pool_before as u128)
            .unwrap() as u64;

        // Execute deposit
        let result = execute_deposit(
            &mut svm,
            &depositor,
            &pool_state_pda,
            &pool_stake_pda,
            &reserve_stake_pda,
            &lst_mint_pda,
            &depositor_lst_ata,
            deposit_amount,
        );
        print_transaction_logs(&result);
        assert!(result.is_ok(), "Deposit should succeed");

        // Get actual minted amount
        let depositor_balance =
            get_token_balance(&svm.get_account(&depositor_lst_ata).unwrap().data);

        // Get mint supply after
        let mint_supply_after = get_mint_supply(&svm.get_account(&lst_mint_pda).unwrap().data);

        eprintln!("\n=== State After Deposit ===");
        eprintln!("  Mint total supply: {}", mint_supply_after);
        eprintln!("  Depositor balance: {}", depositor_balance);
        eprintln!("  Expected increase: {}", expected_lst_minted);
        eprintln!(
            "  Actual increase: {}",
            mint_supply_after - mint_supply_before
        );

        // INVARIANT: mint supply should increase by exactly depositor balance
        assert_eq!(
            mint_supply_after,
            mint_supply_before + depositor_balance,
            "Mint supply should increase by exactly the minted amount. Before: {}, After: {}, Minted: {}",
            mint_supply_before,
            mint_supply_after,
            depositor_balance
        );

        // Verify the minted amount matches expected
        assert_eq!(
            depositor_balance, expected_lst_minted,
            "Minted amount should match expected calculation"
        );

        println!("\n=== Test Passed: lst_supply Invariant Maintained ===");
    }

    #[test]
    fn test_deposit_to_uninitialized_pool_fails() {
        let mut svm = setup_svm();

        // Create PDAs without initializing
        let fake_initializer = Keypair::new();
        let seed = 99999u64;

        let (pool_state_pda, _) = derive_pool_state_pda(&fake_initializer.pubkey(), seed);
        let (lst_mint_pda, _) = derive_lst_mint_pda(&pool_state_pda);
        let (pool_stake_pda, _) = derive_stake_account_pda(&pool_state_pda);
        let (reserve_stake_pda, _) = derive_reserve_stake_account_pda(&pool_state_pda);

        // Create depositor
        let depositor = Keypair::new();
        svm.airdrop(&depositor.pubkey(), 2_000_000_000).unwrap();

        // Create a fake ATA (won't actually work but needed for instruction)
        let depositor_lst_ata = derive_ata(&depositor.pubkey(), &lst_mint_pda);

        // Try to deposit to uninitialized pool
        let instruction_data = create_deposit_instruction_data(1_000_000_000);

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
            data: instruction_data,
        };

        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&depositor.pubkey()),
            &[&depositor],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(transaction);
        print_transaction_logs(&result);

        assert!(result.is_err(), "Deposit to uninitialized pool should fail");

        println!("\n=== Test Passed: Uninitialized Pool Rejected ===");
    }
}

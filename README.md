# Liquid Staking Token (LST) - MVP

A Solana program that allows users to deposit SOL and receive liquid staking tokens (LST) representing their proportional share of a staked pool.

## Overview

Users deposit SOL → receive LST tokens → SOL gets staked to a validator → staking rewards accrue → LST value increases over time.

## Architecture
```
┌─────────────┐     deposit      ┌─────────────┐     stake      ┌─────────────┐
│    User     │ ───────────────► │   Reserve   │ ─────────────► │  Pool Stake │
│             │ ◄─────────────── │   Stake     │                │  (Validator)│
└─────────────┘    LST tokens    └─────────────┘                └─────────────┘
```

## Instructions

### 1. Initialize (Discriminator: 0)
Creates a new staking pool.

**Accounts:**
- `initializer` (signer, mut) - Pool creator, pays for accounts
- `initializer_lst_ata` (mut) - Receives initial LST tokens
- `pool_state` (mut) - PDA storing pool configuration
- `lst_mint` (signer, mut) - LST token mint
- `stake_account` (mut) - Main stake account delegated to validator
- `reserve_stake` (mut) - Reserve for collecting deposits
- `validator_vote` - Validator vote account to delegate to
- Sysvars: clock, rent, stake_history, stake_config
- Programs: system, token, stake, ata

**Data:** `seed (u64)`

**Effect:** Creates pool with 1 SOL minimum stake, mints equivalent LST to initializer.

---

### 2. Deposit (Discriminator: 1)
Deposit SOL to receive LST tokens.

**Accounts:**
- `depositor` (signer, mut) - Deposits SOL
- `pool_state` (mut) - Pool configuration
- `pool_stake` - Main stake account (read lamports)
- `reserve_stake` (mut) - Receives deposited SOL
- `lst_mint` (mut) - Mint LST to depositor
- `depositor_lst_ata` (mut) - Receives LST tokens
- Programs: system, token, stake

**Data:** `amount (u64)`

**Effect:** 
- Transfers SOL to reserve
- Mints LST proportional to: `deposit_amount * lst_supply / total_pool_value`

---

### 3. InitializeReserve (Discriminator: 2)
Initializes and delegates the reserve stake account. Permissionless crank.

**Accounts:**
- `pool_state` (mut)
- `pool_stake` - Main stake account
- `reserve_stake` (mut) - Gets initialized and delegated
- `validator_vote` - Must match pool's validator
- Sysvars: clock, rent, stake_history, stake_config
- Programs: system, stake

**Data:** None (just discriminator)

**Effect:** Initializes reserve as stake account, delegates to validator.

**Requirement:** Reserve must have >= `rent + MIN_STAKE_DELEGATION` lamports.

---

### 4. MergeReserve (Discriminator: 3)
Merges reserve stake into main pool stake. Permissionless crank.

**Accounts:**
- `pool_state`
- `pool_stake` (mut) - Destination
- `reserve_stake` (mut) - Source (gets absorbed)
- Sysvars: clock, stake_history
- Programs: stake

**Data:** None

**Effect:** Combines reserve into pool_stake, closes reserve.

**Requirement:** Both stakes must be active (wait 1+ epoch after InitializeReserve).

---

### 5. Withdraw (Discriminator: 4)
Burns LST and creates a deactivating stake account for the user.

**Accounts:**
- `user` (signer, mut)
- `pool_state` (mut)
- `pool_stake` (mut) - Splits stake from here
- `reserve_stake` - For calculating exchange rate
- `user_stake` (mut) - PDA created for user's withdrawing stake
- `lst_mint` (mut) - Burns LST
- `user_lst_ata` (mut) - Burns from here
- Sysvars: clock, rent, stake_history
- Programs: system, stake, token

**Data:** `amount (u64) | nonce (u64)`

**Effect:**
- Burns user's LST
- Splits SOL from pool_stake to user_stake (proportional to: `lst_amount * total_pool_value / lst_supply`)
- Deactivates user_stake (starts cooldown)

**Note:** User must use unique nonce for each withdraw.

---

### 6. WithdrawComplete (Discriminator: 5)
Claims SOL after stake cooldown completes.

**Accounts:**
- `user` (signer, mut) - Receives SOL
- `pool_state`
- `user_stake` (mut) - Deactivated stake to claim
- Sysvars: clock, stake_history
- Programs: stake

**Data:** `nonce (u64)`

**Effect:** Withdraws all lamports from user_stake to user.

**Requirement:** Must wait ~1 epoch after Withdraw for cooldown.

---

## Exchange Rate
```
exchange_rate = total_pool_value / lst_supply

where:
  total_pool_value = pool_stake.lamports + reserve_stake.lamports
```

- **Deposit:** `lst_received = deposit_amount * lst_supply / total_pool_value`
- **Withdraw:** `sol_received = lst_burned * total_pool_value / lst_supply`

As staking rewards accrue, `total_pool_value` increases while `lst_supply` stays constant → exchange rate increases → 1 LST becomes worth more SOL.

---

## PDA Seeds

| Account | Seeds |
|---------|-------|
| pool_state | `["lst_pool", seed]` |
| stake_account | `["stake", pool_state]` |
| reserve_stake | `["reserve_stake", pool_state]` |
| user_stake | `["withdraw", pool_state, user, nonce]` |

---

## Typical Flow
```
1. Initialize          → Pool created, authority gets initial LST
2. User deposits       → SOL goes to reserve, user gets LST
3. InitializeReserve   → Reserve gets delegated (crank)
4. [wait 1 epoch]
5. MergeReserve        → Reserve merged into pool_stake (crank)
6. [staking rewards accrue over time]
7. User withdraws      → Burns LST, gets user_stake in cooldown
8. [wait 1 epoch]
9. WithdrawComplete    → User claims SOL
```

---

## Building
```bash
cargo build-sbf
```

## Testing
```bash
# Run all tests
cargo test

# Run specific test file
cargo test --test initialize
cargo test --test deposit
cargo test --test withdraw
cargo test --test withdraw_complete
cargo test --test initialize-reserve
cargo test --test merge
```

## Deployment
```bash
solana program deploy target/deploy/liquid_staking_token.so
```

---

## Constants

- `MIN_STAKE_DELEGATION`: 1 SOL (1_000_000_000 lamports)
- `STAKE_ACCOUNT_SIZE`: 200 bytes

---

## Security Considerations

- Pool authority receives initial LST to prevent exchange rate manipulation
- Nonce system prevents double-withdraw attacks
- All account validations use PDA verification
- Exchange rate formula protects against dilution attacks
- Minimum stake requirements prevent dust attacks
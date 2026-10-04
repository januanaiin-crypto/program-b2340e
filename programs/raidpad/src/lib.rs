#![no_std]

use pinocchio::cpi::{Seed, Signer};
use pinocchio::error::ProgramError;
use pinocchio::sysvars::{clock::Clock, rent::Rent, Sysvar};
use pinocchio::{AccountView, Address, ProgramResult};
use pinocchio_system::instructions::{Allocate, Assign, Transfer};

#[cfg(not(feature = "no-entrypoint"))]
pinocchio::program_entrypoint!(process_instruction, 10);
pinocchio::no_allocator!();
pinocchio::nostd_panic_handler!();

pub const ID: Address = Address::from_str_const("B4shPJRpKJx5Cy3nfm8Kgw6LaSDibytkpxLGqus1R5D9");
const SYSTEM: Address = pinocchio_system::ID;
const LOADER: Address = Address::from_str_const("BPFLoaderUpgradeab1e11111111111111111111111");
const PUMP: Address = Address::from_str_const("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P");
const PUMP_FEES: Address = Address::from_str_const("pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ");
const TOKEN: Address = Address::from_str_const("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const TOKEN_2022: Address = Address::from_str_const("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
const WSOL: Address = Address::from_str_const("So11111111111111111111111111111111111111112");
const ZERO: Address = Address::new_from_array([0; 32]);
const SYSVAR: Address = Address::from_str_const("Sysvar1111111111111111111111111111111111111");
// runtime-reserved ids that are neither executable nor sysvars on mainnet, yet always demoted to read-only
const RESERVED: [Address; 5] = [
    Address::from_str_const("StakeConfig11111111111111111111111111111111"),
    Address::from_str_const("LoaderV411111111111111111111111111111111111"),
    Address::from_str_const("ZkTokenProof1111111111111111111111111111111"),
    Address::from_str_const("NativeLoader1111111111111111111111111111111"),
    SYSVAR,
];
const CONFIG_LEN: usize = 187;
const TOKEN_LEN: usize = 205;
const TOMB_LEN: usize = 41;
const RECEIPT_LEN: usize = 41;
const CONFIG_DISC: [u8; 8] = [155, 12, 170, 224, 30, 250, 204, 130];
const TOKEN_DISC: [u8; 8] = [131, 254, 39, 144, 4, 179, 134, 127];
const TOMB_DISC: [u8; 8] = [48, 213, 223, 166, 145, 54, 178, 183];
const RECEIPT_DISC: [u8; 8] = [39, 154, 73, 106, 80, 102, 145, 153];
const SC_DISC: [u8; 8] = [216, 74, 9, 0, 56, 140, 93, 75];
const BC_DISC: [u8; 8] = [23, 183, 248, 55, 96, 216, 172, 96];
const HARD_GLOBAL_CAP: u64 = 2_000_000_000;
const HARD_WALLET_CEILING: u64 = 500_000_000;
const TIER_CAPS: [u64; 4] = [0, 10_000_000, 30_000_000, 100_000_000];

// Anchor/Borsh offsets include the discriminator; there is no Rust padding.
mod c {
    pub const ADMIN: usize = 8;
    pub const PENDING: usize = 40;
    pub const OPERATOR: usize = 72;
    pub const TREASURY: usize = 104;
    pub const PAUSED: usize = 136;
    pub const CLOSING: usize = 137;
    pub const CAP: usize = 138;
    pub const ALLOWANCE: usize = 146;
    pub const LAST: usize = 154;
    pub const CEILING: usize = 162;
    pub const OPEN: usize = 170;
    pub const TOTAL: usize = 178;
    pub const BUMP: usize = 186;
}
mod t {
    pub const MINT: usize = 8;
    pub const CREATOR: usize = 40;
    pub const WALLET: usize = 72;
    pub const PENDING: usize = 104;
    pub const BPS: usize = 136;
    pub const TIER: usize = 138;
    pub const ALLOWANCE: usize = 139;
    pub const LAST: usize = 147;
    pub const RELEASED: usize = 155;
    pub const REGISTERED: usize = 163;
    pub const PAYER: usize = 171;
    pub const VAULT_BUMP: usize = 203;
    pub const BUMP: usize = 204;
}
#[repr(u32)]
enum Error {
    Unauthorized = 6000,
    Paused,
    BadParams,
    BadPumpAccount,
    BadShares,
    MathOverflow,
    NotPaused,
    WindingDown,
    NotWindingDown,
    TokensOpen,
    NoPending,
    TokenAllowance,
    GlobalAllowance,
    WalletCeiling,
    InsufficientFunds,
    BadVault,
    VaultBelowRent,
    BadWallet,
    PermanentlyClosed = 6019,
    DuplicateAccounts,
    NoReceipt,
}
fn err(e: Error) -> ProgramError {
    ProgramError::Custom(e as u32)
}
fn need(ok: bool, e: Error) -> ProgramResult {
    if ok {
        Ok(())
    } else {
        Err(err(e))
    }
}
fn rd<const N: usize>(d: &[u8], o: usize) -> Result<[u8; N], ProgramError> {
    let end = o.checked_add(N).ok_or(ProgramError::InvalidAccountData)?;
    d.get(o..end)
        .ok_or(ProgramError::InvalidAccountData)?
        .try_into()
        .map_err(|_| ProgramError::InvalidAccountData)
}
fn byte(d: &[u8], o: usize) -> Result<u8, ProgramError> {
    d.get(o).copied().ok_or(ProgramError::InvalidAccountData)
}
fn key(d: &[u8], o: usize) -> Result<Address, ProgramError> {
    Ok(Address::new_from_array(rd(d, o)?))
}
fn u64_at(d: &[u8], o: usize) -> Result<u64, ProgramError> {
    Ok(u64::from_le_bytes(rd(d, o)?))
}
fn put(d: &mut [u8], o: usize, v: &[u8]) -> ProgramResult {
    let end = o
        .checked_add(v.len())
        .ok_or(ProgramError::InvalidAccountData)?;
    d.get_mut(o..end)
        .ok_or(ProgramError::InvalidAccountData)?
        .copy_from_slice(v);
    Ok(())
}
fn add(a: u64, b: u64) -> Result<u64, ProgramError> {
    a.checked_add(b).ok_or(err(Error::MathOverflow))
}
fn sub(a: u64, b: u64) -> Result<u64, ProgramError> {
    a.checked_sub(b).ok_or(err(Error::MathOverflow))
}
fn account(a: &[AccountView], i: usize) -> Result<&AccountView, ProgramError> {
    a.get(i).ok_or(ProgramError::NotEnoughAccountKeys)
}
fn writable(a: &AccountView) -> ProgramResult {
    need(a.is_writable(), Error::Unauthorized)
}
fn signer(a: &AccountView) -> ProgramResult {
    if a.is_signer() {
        Ok(())
    } else {
        Err(ProgramError::MissingRequiredSignature)
    }
}
fn system(a: &AccountView) -> ProgramResult {
    need(a.address() == &SYSTEM && a.executable(), Error::BadVault)
}
fn find(seeds: &[&[u8]], a: &AccountView) -> Result<u8, ProgramError> {
    let (address, bump) = Address::find_program_address(seeds, &ID);
    need(a.address() == &address, Error::BadVault)?;
    Ok(bump)
}
fn derived(seeds: &[&[u8]]) -> Result<Address, ProgramError> {
    Address::create_program_address(seeds, &ID).map_err(|_| err(Error::BadVault))
}
fn check_pda(a: &AccountView, seeds: &[&[u8]]) -> ProgramResult {
    need(a.address() == &derived(seeds)?, Error::BadVault)
}
fn money(a: &AccountView, seeds: &[&[u8]]) -> ProgramResult {
    check_pda(a, seeds)?;
    need(a.owned_by(&SYSTEM) && a.data_len() == 0, Error::BadVault)
}
fn load<const N: usize>(a: &AccountView, disc: &[u8; 8]) -> Result<[u8; N], ProgramError> {
    if !a.owned_by(&ID) {
        return Err(ProgramError::IllegalOwner);
    }
    if a.data_len() != N {
        return Err(ProgramError::InvalidAccountData);
    }
    let data = a.try_borrow()?;
    if rd::<8>(&data, 0)? != *disc {
        return Err(ProgramError::InvalidAccountData);
    }
    rd(&data, 0)
}
fn save(a: &mut AccountView, d: &[u8]) -> ProgramResult {
    writable(a)?;
    let mut data = a.try_borrow_mut()?;
    if data.len() != d.len() {
        return Err(ProgramError::InvalidAccountData);
    }
    data.copy_from_slice(d);
    Ok(())
}
fn auth(d: &[u8], offset: usize, a: &AccountView) -> ProgramResult {
    signer(a)?;
    need(a.address() == &key(d, offset)?, Error::Unauthorized)
}
fn now() -> Result<i64, ProgramError> {
    Ok(Clock::get()?.unix_timestamp)
}
fn rent(len: usize) -> Result<u64, ProgramError> {
    Rent::get()?.try_minimum_balance(len)
}
fn transfer(from: &AccountView, to: &AccountView, lamports: u64, seeds: &[Seed]) -> ProgramResult {
    writable(from)?;
    writable(to)?;
    if lamports == 0 {
        return Ok(());
    }
    let ix = Transfer { from, to, lamports };
    if seeds.is_empty() {
        ix.invoke()
    } else {
        ix.invoke_signed(&[Signer::from(seeds)])
    }
}
fn init_account(
    payer: &AccountView,
    new: &AccountView,
    len: usize,
    seeds: &[Seed],
) -> ProgramResult {
    signer(payer)?;
    writable(payer)?;
    writable(new)?;
    // Never overwrite existing records; prefunding an empty system PDA remains harmless.
    if !new.owned_by(&SYSTEM) || new.data_len() != 0 {
        return Err(ProgramError::AccountAlreadyInitialized);
    }
    let shortfall = rent(len)?.saturating_sub(new.lamports());
    transfer(payer, new, shortfall, &[])?;
    allocate(new, len, seeds)
}
fn allocate(new: &AccountView, len: usize, seeds: &[Seed]) -> ProgramResult {
    let signers = [Signer::from(seeds)];
    Allocate {
        account: new,
        space: len as u64,
    }
    .invoke_signed(&signers)?;
    Assign {
        account: new,
        owner: &ID,
    }
    .invoke_signed(&signers)
}
fn close_account(a: &mut [AccountView], from: usize, to: usize) -> ProgramResult {
    let src = account(a, from)?;
    let dst = account(a, to)?;
    writable(src)?;
    writable(dst)?;
    need(src.address() != dst.address(), Error::BadWallet)?;
    let balance = add(dst.lamports(), src.lamports())?;
    let src = a.get_mut(from).ok_or(ProgramError::NotEnoughAccountKeys)?;
    src.try_borrow_mut()?.fill(0);
    // close() sets length=0, lamports=0 and owner=system before any later instruction/CPI.
    src.close()?;
    a.get_mut(to)
        .ok_or(ProgramError::NotEnoughAccountKeys)?
        .set_lamports(balance);
    Ok(())
}
fn emit(disc: [u8; 8], fields: &[&[u8]]) -> ProgramResult {
    let mut d = [0u8; 160];
    put(&mut d, 0, &disc)?;
    let mut len = 8usize;
    for field in fields {
        put(&mut d, len, field)?;
        len = len
            .checked_add(field.len())
            .ok_or(err(Error::MathOverflow))?;
    }
    #[cfg(target_os = "solana")]
    {
        let raw = [(d.as_ptr(), len as u64)];
        // SAFETY: the slice is initialized and lives through this syscall.
        unsafe { pinocchio::syscalls::sol_log_data(raw.as_ptr() as *const u8, 1) };
    }
    Ok(())
}

fn empty(a: &AccountView) -> ProgramResult {
    need(
        a.owned_by(&SYSTEM) && a.data_len() == 0 && !a.executable(),
        Error::BadParams,
    )
}
fn config(a: &AccountView) -> Result<[u8; CONFIG_LEN], ProgramError> {
    let d = load(a, &CONFIG_DISC)?;
    check_pda(a, &[b"config", &[byte(&d, c::BUMP)?]])?;
    need(
        byte(&d, c::PAUSED)? <= 1 && byte(&d, c::CLOSING)? <= 1,
        Error::BadParams,
    )?;
    Ok(d)
}
fn token(a: &AccountView) -> Result<[u8; TOKEN_LEN], ProgramError> {
    let d = load(a, &TOKEN_DISC)?;
    check_pda(
        a,
        &[b"token", key(&d, t::MINT)?.as_ref(), &[byte(&d, t::BUMP)?]],
    )?;
    tier_cap(byte(&d, t::TIER)?)?;
    Ok(d)
}
fn live(d: &[u8]) -> ProgramResult {
    need(byte(d, c::CLOSING)? == 0, Error::WindingDown)?;
    need(byte(d, c::PAUSED)? == 0, Error::Paused)
}
fn tier_cap(tier: u8) -> Result<u64, ProgramError> {
    TIER_CAPS
        .get(usize::from(tier))
        .copied()
        .ok_or(err(Error::BadParams))
}
fn bps_valid(bps: u16) -> ProgramResult {
    need(
        (500..=8000).contains(&bps) && bps % 100 == 0,
        Error::BadParams,
    )
}
fn refill(allowance: u64, last: i64, cap: u64, ts: i64) -> Result<u64, ProgramError> {
    let elapsed = if ts > last {
        ts.checked_sub(last).ok_or(err(Error::MathOverflow))? as u128
    } else {
        0
    };
    let added = u128::from(cap)
        .checked_mul(elapsed)
        .ok_or(err(Error::MathOverflow))?
        / 86400;
    let value = u128::from(allowance)
        .checked_add(added)
        .ok_or(err(Error::MathOverflow))?;
    Ok(value.min(u128::from(cap / 4)) as u64)
}
fn refill_at(d: &mut [u8], allowance: usize, last: usize, cap: u64, ts: i64) -> ProgramResult {
    let previous = i64::from_le_bytes(rd(d, last)?);
    let value = refill(u64_at(d, allowance)?, previous, cap, ts)?;
    put(d, allowance, &value.to_le_bytes())?;
    // Keep the high-water timestamp on clock rollback: the same interval cannot refill twice.
    put(d, last, &ts.max(previous).to_le_bytes())
}
fn wallet(
    a: &AccountView,
    c: &[u8],
    t: &[u8],
    config: &Address,
    token: &Address,
    vault: &Address,
) -> ProgramResult {
    let k = a.address();
    plain_wallet(a)?;
    need(
        *k != key(t, t::CREATOR)?
            && *k != key(c, c::TREASURY)?
            && k != config
            && k != token
            && k != vault
            && *k != Address::find_program_address(&[b"tomb"], &ID).0
            && *k != Address::find_program_address(&[b"receipt", key(t, t::MINT)?.as_ref()], &ID).0,
        Error::BadWallet,
    )
}
fn plain_wallet(a: &AccountView) -> ProgramResult {
    need(a.owned_by(&SYSTEM) && a.data_len() == 0, Error::BadWallet)?;
    safe_recipient(a)
}
fn safe_recipient(a: &AccountView) -> ProgramResult {
    let k = a.address();
    need(
        !a.executable()
            && *k != ZERO
            && !RESERVED.contains(k)
            && ![
                ID,
                LOADER,
                PUMP,
                PUMP_FEES,
                TOKEN,
                TOKEN_2022,
                WSOL,
                Address::from_str_const("ComputeBudget111111111111111111111111111111"),
                Address::from_str_const("AddressLookupTab1e1111111111111111111111111"),
            ]
            .contains(k),
        Error::BadWallet,
    )
}
fn creator_destination(a: &AccountView, mint: &Address, treasury: &Address) -> ProgramResult {
    // Admission requires a plain wallet; fixed-address payouts permit changed owner/data.
    safe_recipient(a)?;
    need(a.address() != treasury, Error::BadWallet)?;
    for seeds in [
        &[b"config".as_slice()][..],
        &[b"tomb".as_slice()][..],
        &[b"token".as_slice(), mint.as_ref()][..],
        &[b"vault".as_slice(), mint.as_ref()][..],
        &[b"receipt".as_slice(), mint.as_ref()][..],
    ] {
        need(
            *a.address() != Address::find_program_address(seeds, &ID).0,
            Error::BadWallet,
        )?;
    }
    Ok(())
}
fn distinct(a: &[AccountView]) -> ProgramResult {
    for (i, item) in a.iter().enumerate() {
        need(
            !a[..i].iter().any(|other| other.address() == item.address()),
            Error::DuplicateAccounts,
        )?;
    }
    Ok(())
}

pub fn process_instruction(pid: &Address, a: &mut [AccountView], d: &[u8]) -> ProgramResult {
    need(pid == &ID, Error::Unauthorized)?;
    let disc = rd::<8>(d, 0).map_err(|_| ProgramError::InvalidInstructionData)?;
    let args = d.get(8..).ok_or(ProgramError::InvalidInstructionData)?;
    let (len, handler): (usize, fn(&mut [AccountView], &[u8]) -> ProgramResult) = match disc {
        [175, 175, 109, 31, 13, 152, 155, 237] => (112, initialize),
        [32, 146, 36, 240, 80, 183, 36, 84] => (2, register),
        [253, 249, 15, 206, 28, 127, 193, 241] => (8, release),
        [197, 250, 253, 88, 170, 70, 237, 66] => (33, set_tier),
        [227, 190, 134, 7, 242, 238, 103, 48] => (32, |a, d| rotation(a, d, 0)),
        [127, 218, 141, 52, 39, 51, 234, 76] => (0, |a, d| rotation(a, d, 1)),
        [135, 151, 64, 124, 50, 13, 169, 139] => (0, |a, d| rotation(a, d, 2)),
        [91, 60, 125, 192, 176, 225, 166, 218] => (1, |a, d| admin(a, d, 0)),
        [238, 153, 101, 169, 243, 131, 36, 1] => (32, |a, d| admin(a, d, 1)),
        [59, 13, 205, 253, 179, 219, 157, 189] => (8, |a, d| admin(a, d, 2)),
        [121, 214, 199, 212, 87, 39, 117, 234] => (32, |a, d| admin(a, d, 3)),
        [112, 42, 45, 90, 116, 181, 13, 170] => (0, |a, d| admin(a, d, 4)),
        [89, 28, 134, 161, 28, 9, 243, 171] => (0, |a, d| admin(a, d, 5)),
        [235, 80, 65, 220, 37, 85, 153, 42] => (8, |a, d| admin(a, d, 6)),
        [85, 39, 237, 113, 69, 80, 31, 15] => (0, settle),
        [2, 24, 91, 14, 203, 173, 249, 200] => (0, late_refund),
        [145, 9, 72, 157, 95, 125, 61, 85] => (0, close_config),
        _ => return Err(ProgramError::InvalidInstructionData),
    };
    if args.len() != len {
        return Err(ProgramError::InvalidInstructionData);
    }
    handler(a, args)
}

fn initialize(a: &mut [AccountView], d: &[u8]) -> ProgramResult {
    let authority = account(a, 0)?;
    signer(authority)?;
    writable(authority)?;
    writable(account(a, 1)?)?;
    system(account(a, 5)?)?;
    // Tomb makes closure permanent even if the upgrade authority tries to reinitialize.
    find(&[b"tomb"], account(a, 4)?)?;
    need(
        account(a, 4)?.owned_by(&SYSTEM) && account(a, 4)?.data_len() == 0,
        Error::PermanentlyClosed,
    )?;
    let program = account(a, 2)?;
    let pd = account(a, 3)?;
    need(
        program.address() == &ID && program.owned_by(&LOADER) && program.executable(),
        Error::Unauthorized,
    )?;
    let pd_key = Address::find_program_address(&[ID.as_ref()], &LOADER).0;
    need(
        pd.address() == &pd_key && pd.owned_by(&LOADER),
        Error::Unauthorized,
    )?;
    {
        let data = program.try_borrow()?;
        need(
            rd::<4>(&data, 0)? == 2u32.to_le_bytes() && key(&data, 4)? == pd_key,
            Error::Unauthorized,
        )?;
        let data = pd.try_borrow()?;
        need(
            rd::<4>(&data, 0)? == 3u32.to_le_bytes()
                && byte(&data, 12)? == 1
                && key(&data, 13)? == *authority.address(),
            Error::Unauthorized,
        )?;
    }
    let admin = key(d, 0)?;
    let operator = key(d, 32)?;
    let treasury = key(d, 64)?;
    need(
        admin != ZERO && operator != ZERO && treasury != ZERO,
        Error::BadParams,
    )?;
    let cap = u64_at(d, 96)?;
    let ceiling = u64_at(d, 104)?;
    need(
        cap <= HARD_GLOBAL_CAP && ceiling <= HARD_WALLET_CEILING,
        Error::BadParams,
    )?;
    let bump = [find(&[b"config"], account(a, 1)?)?];
    init_account(
        authority,
        account(a, 1)?,
        CONFIG_LEN,
        &[Seed::from(b"config"), Seed::from(&bump)],
    )?;
    let mut c = [0u8; CONFIG_LEN];
    put(&mut c, 0, &CONFIG_DISC)?;
    put(&mut c, c::ADMIN, admin.as_ref())?;
    put(&mut c, c::OPERATOR, operator.as_ref())?;
    put(&mut c, c::TREASURY, treasury.as_ref())?;
    put(&mut c, c::CAP, &cap.to_le_bytes())?;
    put(&mut c, c::ALLOWANCE, &0u64.to_le_bytes())?;
    put(&mut c, c::LAST, &now()?.to_le_bytes())?;
    put(&mut c, c::CEILING, &ceiling.to_le_bytes())?;
    put(&mut c, c::BUMP, &bump)?;
    save(a.get_mut(1).ok_or(ProgramError::NotEnoughAccountKeys)?, &c)?;
    emit(
        [208, 213, 115, 98, 115, 82, 201, 209],
        &[
            admin.as_ref(),
            operator.as_ref(),
            treasury.as_ref(),
            &cap.to_le_bytes(),
            &ceiling.to_le_bytes(),
        ],
    )
}

fn register(a: &mut [AccountView], d: &[u8]) -> ProgramResult {
    let bps = u16::from_le_bytes(rd(d, 0)?);
    bps_valid(bps)?;
    let tier = 0u8;
    let mut c = config(account(a, 1)?)?;
    auth(&c, c::OPERATOR, account(a, 0)?)?;
    live(&c)?;
    for i in 0..3 {
        writable(account(a, i)?)?;
    }
    system(account(a, 9)?)?;
    let mint = *account(a, 4)?.address();
    let bump = [find(&[b"token", mint.as_ref()], account(a, 2)?)?];
    let vb = [find(&[b"vault", mint.as_ref()], account(a, 3)?)?];
    money(account(a, 3)?, &[b"vault", mint.as_ref(), &vb])?;
    need(account(a, 3)?.lamports() >= rent(0)?, Error::VaultBelowRent)?;
    let (creator, actual_bps) = read_shares(
        account(a, 4)?,
        account(a, 5)?,
        account(a, 3)?.address(),
        &key(&c, c::TREASURY)?,
    )?;
    need(
        actual_bps == bps && account(a, 7)?.address() == &creator,
        Error::BadShares,
    )?;
    read_curve(account(a, 4)?, account(a, 5)?, account(a, 6)?)?;
    plain_wallet(account(a, 7)?)?;
    creator_destination(account(a, 7)?, &mint, &key(&c, c::TREASURY)?)?;
    // Fixed refund roles must be distinct because settlement rejects duplicate keys.
    need(creator != *account(a, 0)?.address(), Error::BadWallet)?;
    let mut t = [0u8; TOKEN_LEN];
    put(&mut t, 0, &TOKEN_DISC)?;
    put(&mut t, t::MINT, mint.as_ref())?;
    put(&mut t, t::CREATOR, creator.as_ref())?;
    wallet(
        account(a, 8)?,
        &c,
        &t,
        account(a, 1)?.address(),
        account(a, 2)?.address(),
        account(a, 3)?.address(),
    )?;
    signer(account(a, 8)?)?;
    put(&mut t, t::WALLET, account(a, 8)?.address().as_ref())?;
    put(&mut t, t::BPS, &bps.to_le_bytes())?;
    put(&mut t, t::TIER, &[tier])?;
    let ts = now()?;
    put(&mut t, t::LAST, &ts.to_le_bytes())?;
    put(&mut t, t::REGISTERED, &ts.to_le_bytes())?;
    put(&mut t, t::PAYER, account(a, 0)?.address().as_ref())?;
    put(&mut t, t::VAULT_BUMP, &vb)?;
    put(&mut t, t::BUMP, &bump)?;
    init_account(
        account(a, 0)?,
        account(a, 2)?,
        TOKEN_LEN,
        &[
            Seed::from(b"token"),
            Seed::from(mint.as_ref()),
            Seed::from(&bump),
        ],
    )?;
    let open = add(u64_at(&c, c::OPEN)?, 1)?;
    let total = add(u64_at(&c, c::TOTAL)?, 1)?;
    put(&mut c, c::OPEN, &open.to_le_bytes())?;
    put(&mut c, c::TOTAL, &total.to_le_bytes())?;
    save(a.get_mut(1).ok_or(ProgramError::NotEnoughAccountKeys)?, &c)?;
    save(a.get_mut(2).ok_or(ProgramError::NotEnoughAccountKeys)?, &t)?;
    emit(
        [210, 38, 249, 182, 79, 112, 240, 225],
        &[
            mint.as_ref(),
            creator.as_ref(),
            account(a, 8)?.address().as_ref(),
            &bps.to_le_bytes(),
            &[tier],
        ],
    )
}

fn release(a: &mut [AccountView], d: &[u8]) -> ProgramResult {
    let mut c = config(account(a, 1)?)?;
    auth(&c, c::OPERATOR, account(a, 0)?)?;
    live(&c)?;
    let mut t = token(account(a, 2)?)?;
    for i in 1..5 {
        writable(account(a, i)?)?;
    }
    system(account(a, 5)?)?;
    let mint = key(&t, t::MINT)?;
    let vb = [byte(&t, t::VAULT_BUMP)?];
    money(account(a, 3)?, &[b"vault", mint.as_ref(), &vb])?;
    need(
        account(a, 4)?.address() == &key(&t, t::WALLET)?,
        Error::Unauthorized,
    )?;
    wallet(
        account(a, 4)?,
        &c,
        &t,
        account(a, 1)?.address(),
        account(a, 2)?.address(),
        account(a, 3)?.address(),
    )?;
    let amount = u64_at(d, 0)?;
    need(amount > 0, Error::BadParams)?;
    let ts = now()?;
    let cap = tier_cap(byte(&t, t::TIER)?)?;
    let global_cap = u64_at(&c, c::CAP)?;
    refill_at(&mut t, t::ALLOWANCE, t::LAST, cap, ts)?;
    refill_at(&mut c, c::ALLOWANCE, c::LAST, global_cap, ts)?;
    need(amount <= u64_at(&t, t::ALLOWANCE)?, Error::TokenAllowance)?;
    need(amount <= u64_at(&c, c::ALLOWANCE)?, Error::GlobalAllowance)?;
    need(
        amount
            <= account(a, 3)?
                .lamports()
                .checked_sub(rent(0)?)
                .ok_or(err(Error::VaultBelowRent))?,
        Error::InsufficientFunds,
    )?;
    need(
        add(account(a, 4)?.lamports(), amount)? <= u64_at(&c, c::CEILING)?,
        Error::WalletCeiling,
    )?;
    let left = sub(u64_at(&t, t::ALLOWANCE)?, amount)?;
    let global_left = sub(u64_at(&c, c::ALLOWANCE)?, amount)?;
    let total = add(u64_at(&t, t::RELEASED)?, amount)?;
    put(&mut t, t::ALLOWANCE, &left.to_le_bytes())?;
    put(&mut t, t::RELEASED, &total.to_le_bytes())?;
    put(&mut c, c::ALLOWANCE, &global_left.to_le_bytes())?;
    transfer(
        account(a, 3)?,
        account(a, 4)?,
        amount,
        &[
            Seed::from(b"vault"),
            Seed::from(mint.as_ref()),
            Seed::from(&vb),
        ],
    )?;
    save(a.get_mut(1).ok_or(ProgramError::NotEnoughAccountKeys)?, &c)?;
    save(a.get_mut(2).ok_or(ProgramError::NotEnoughAccountKeys)?, &t)?;
    emit(
        [232, 229, 255, 136, 101, 189, 15, 220],
        &[
            mint.as_ref(),
            &amount.to_le_bytes(),
            account(a, 4)?.address().as_ref(),
            &left.to_le_bytes(),
        ],
    )
}

fn set_tier(a: &mut [AccountView], d: &[u8]) -> ProgramResult {
    let c = config(account(a, 1)?)?;
    need(byte(&c, c::CLOSING)? == 0, Error::WindingDown)?;
    let mut t = token(account(a, 2)?)?;
    auth(&t, t::CREATOR, account(a, 0)?)?;
    need(key(d, 0)? == key(&t, t::WALLET)?, Error::Unauthorized)?;
    let tier = byte(d, 32)?;
    let cap = tier_cap(tier)?;
    let old_cap = tier_cap(byte(&t, t::TIER)?)?;
    refill_at(&mut t, t::ALLOWANCE, t::LAST, old_cap, now()?)?;
    let allowance = u64_at(&t, t::ALLOWANCE)?.min(cap / 4);
    put(&mut t, t::ALLOWANCE, &allowance.to_le_bytes())?;
    put(&mut t, t::TIER, &[tier])?;
    save(a.get_mut(2).ok_or(ProgramError::NotEnoughAccountKeys)?, &t)?;
    emit(
        [195, 162, 76, 118, 218, 200, 225, 238],
        &[
            key(&t, t::MINT)?.as_ref(),
            &[tier],
            &allowance.to_le_bytes(),
        ],
    )
}

fn rotation(a: &mut [AccountView], d: &[u8], mode: u8) -> ProgramResult {
    let c = config(account(a, 1)?)?;
    let mut t = token(account(a, 2)?)?;
    if mode == 1 {
        auth(&t, t::CREATOR, account(a, 0)?)?;
    } else {
        auth(&c, c::ADMIN, account(a, 0)?)?;
    }
    let mint = key(&t, t::MINT)?;
    let vault = derived(&[b"vault", mint.as_ref(), &[byte(&t, t::VAULT_BUMP)?]])?;
    let (event_disc, target) = match mode {
        0 => {
            let new = key(d, 0)?;
            need(account(a, 3)?.address() == &new, Error::BadWallet)?;
            wallet(
                account(a, 3)?,
                &c,
                &t,
                account(a, 1)?.address(),
                account(a, 2)?.address(),
                &vault,
            )?;
            put(&mut t, t::PENDING, new.as_ref())?;
            ([66, 76, 27, 12, 19, 53, 124, 38], new)
        }
        1 => {
            let pending = key(&t, t::PENDING)?;
            need(pending != ZERO, Error::NoPending)?;
            need(account(a, 3)?.address() == &pending, Error::BadWallet)?;
            wallet(
                account(a, 3)?,
                &c,
                &t,
                account(a, 1)?.address(),
                account(a, 2)?.address(),
                &vault,
            )?;
            put(&mut t, t::WALLET, pending.as_ref())?;
            put(&mut t, t::PENDING, ZERO.as_ref())?;
            put(&mut t, t::ALLOWANCE, &0u64.to_le_bytes())?;
            let last = i64::from_le_bytes(rd(&t, t::LAST)?);
            put(&mut t, t::LAST, &now()?.max(last).to_le_bytes())?;
            ([53, 227, 132, 163, 50, 10, 219, 181], pending)
        }
        _ => {
            let pending = key(&t, t::PENDING)?;
            need(pending != ZERO, Error::NoPending)?;
            put(&mut t, t::PENDING, ZERO.as_ref())?;
            ([223, 65, 180, 159, 177, 145, 184, 96], pending)
        }
    };
    save(a.get_mut(2).ok_or(ProgramError::NotEnoughAccountKeys)?, &t)?;
    emit(event_disc, &[mint.as_ref(), target.as_ref()])
}

fn admin(a: &mut [AccountView], d: &[u8], mode: u8) -> ProgramResult {
    let mut c = config(account(a, 1)?)?;
    auth(
        &c,
        if mode == 4 { c::PENDING } else { c::ADMIN },
        account(a, 0)?,
    )?;
    match mode {
        0 => {
            let paused = byte(d, 0)?;
            need(paused <= 1, Error::BadParams)?;
            need(
                paused == 1 || byte(&c, c::CLOSING)? == 0,
                Error::WindingDown,
            )?;
            put(&mut c, c::PAUSED, &[paused])?;
            emit([172, 248, 5, 253, 49, 255, 255, 232], &[&[paused]])?;
        }
        1 | 3 => {
            let k = key(d, 0)?;
            need(k != ZERO, Error::BadParams)?;
            put(
                &mut c,
                if mode == 1 { c::OPERATOR } else { c::PENDING },
                k.as_ref(),
            )?;
            emit(
                if mode == 1 {
                    [187, 242, 164, 221, 208, 246, 180, 178]
                } else {
                    [129, 249, 226, 227, 199, 82, 110, 243]
                },
                &[k.as_ref()],
            )?;
        }
        2 => {
            let cap = u64_at(d, 0)?;
            need(cap <= HARD_GLOBAL_CAP, Error::BadParams)?;
            let old_cap = u64_at(&c, c::CAP)?;
            refill_at(&mut c, c::ALLOWANCE, c::LAST, old_cap, now()?)?;
            let allowance = u64_at(&c, c::ALLOWANCE)?.min(cap / 4);
            put(&mut c, c::ALLOWANCE, &allowance.to_le_bytes())?;
            put(&mut c, c::CAP, &cap.to_le_bytes())?;
            emit(
                [139, 186, 239, 140, 211, 15, 216, 158],
                &[&cap.to_le_bytes(), &allowance.to_le_bytes()],
            )?;
        }
        4 => {
            let k = key(&c, c::PENDING)?;
            need(k != ZERO, Error::NoPending)?;
            put(&mut c, c::ADMIN, k.as_ref())?;
            put(&mut c, c::PENDING, ZERO.as_ref())?;
            emit([174, 12, 76, 139, 158, 99, 110, 254], &[k.as_ref()])?;
        }
        5 => {
            need(byte(&c, c::PAUSED)? == 1, Error::NotPaused)?;
            put(&mut c, c::CLOSING, &[1])?;
            emit([61, 218, 238, 42, 156, 139, 201, 227], &[])?;
        }
        _ => {
            let ceiling = u64_at(d, 0)?;
            need(ceiling <= HARD_WALLET_CEILING, Error::BadParams)?;
            put(&mut c, c::CEILING, &ceiling.to_le_bytes())?;
            emit(
                [252, 243, 51, 249, 81, 164, 37, 17],
                &[&ceiling.to_le_bytes()],
            )?;
        }
    }
    save(a.get_mut(1).ok_or(ProgramError::NotEnoughAccountKeys)?, &c)
}

fn settle(a: &mut [AccountView], _d: &[u8]) -> ProgramResult {
    distinct(a)?;
    let mut c = config(account(a, 0)?)?;
    need(byte(&c, c::CLOSING)? == 1, Error::NotWindingDown)?;
    // Permissionless once closing. Every recipient is authenticated by the live record.
    let t = token(account(a, 1)?)?;
    for i in [0, 1, 2, 4, 5] {
        writable(account(a, i)?)?;
    }
    system(account(a, 6)?)?;
    let mint = key(&t, t::MINT)?;
    let vb = [byte(&t, t::VAULT_BUMP)?];
    money(account(a, 2)?, &[b"vault", mint.as_ref(), &vb])?;
    let creator = key(&t, t::CREATOR)?;
    need(
        account(a, 3)?.address() == &creator && account(a, 4)?.address() == &key(&t, t::PAYER)?,
        Error::Unauthorized,
    )?;
    let treasury = key(&c, c::TREASURY)?;
    let reason = if account(a, 3)?.executable() {
        2u8
    } else if !account(a, 3)?.is_writable() {
        1u8
    } else if creator_destination(account(a, 3)?, &mint, &treasury).is_err() {
        3u8
    } else {
        0u8
    };
    let recipient = if reason == 0 {
        3
    } else {
        // Treasury may also be the original rent payer. Reuse its meta instead of duplicating it.
        let index = if account(a, 4)?.address() == &treasury {
            4
        } else {
            7
        };
        need(
            account(a, index)?.address() == &treasury,
            Error::Unauthorized,
        )?;
        writable(account(a, index)?)?;
        safe_recipient(account(a, index)?)?;
        index
    };
    let rb = [find(&[b"receipt", mint.as_ref()], account(a, 5)?)?];
    let receipt = account(a, 5)?;
    if !receipt.owned_by(&SYSTEM) || receipt.data_len() != 0 {
        return Err(ProgramError::AccountAlreadyInitialized);
    }
    let shortfall = rent(RECEIPT_LEN)?.saturating_sub(receipt.lamports());
    let remaining = sub(account(a, 1)?.lamports(), shortfall)?;
    let funded = add(receipt.lamports(), shortfall)?;
    allocate(
        account(a, 5)?,
        RECEIPT_LEN,
        &[
            Seed::from(b"receipt"),
            Seed::from(mint.as_ref()),
            Seed::from(&rb),
        ],
    )?;
    // The program-owned Token funds the permanent receipt; its remaining rent returns to payer.
    // Allocate/assign first: synchronizing only a credited receipt in CPI would omit Token's debit.
    a.get_mut(1)
        .ok_or(ProgramError::NotEnoughAccountKeys)?
        .set_lamports(remaining);
    a.get_mut(5)
        .ok_or(ProgramError::NotEnoughAccountKeys)?
        .set_lamports(funded);
    let mut r = [0u8; RECEIPT_LEN];
    put(&mut r, 0, &RECEIPT_DISC)?;
    put(&mut r, 8, creator.as_ref())?;
    put(&mut r, 40, &rb)?;
    save(a.get_mut(5).ok_or(ProgramError::NotEnoughAccountKeys)?, &r)?;
    let amount = account(a, 2)?.lamports().saturating_sub(rent(0)?);
    transfer(
        account(a, 2)?,
        account(a, recipient)?,
        amount,
        &[
            Seed::from(b"vault"),
            Seed::from(mint.as_ref()),
            Seed::from(&vb),
        ],
    )?;
    let open = sub(u64_at(&c, c::OPEN)?, 1)?;
    put(&mut c, c::OPEN, &open.to_le_bytes())?;
    save(a.get_mut(0).ok_or(ProgramError::NotEnoughAccountKeys)?, &c)?;
    close_account(a, 1, 4)?;
    if reason != 0 {
        emit(
            [199, 57, 69, 61, 203, 70, 90, 119],
            &[
                mint.as_ref(),
                creator.as_ref(),
                treasury.as_ref(),
                &amount.to_le_bytes(),
                &[reason],
            ],
        )?;
    }
    emit(
        [232, 210, 40, 17, 142, 124, 145, 238],
        &[mint.as_ref(), creator.as_ref(), &amount.to_le_bytes()],
    )
}

fn late_refund(a: &mut [AccountView], _d: &[u8]) -> ProgramResult {
    distinct(a)?;
    find(&[b"config"], account(a, 3)?)?;
    find(&[b"tomb"], account(a, 4)?)?;
    let treasury = if account(a, 3)?.owned_by(&ID) {
        let c = config(account(a, 3)?)?;
        need(byte(&c, c::CLOSING)? == 1, Error::NotWindingDown)?;
        key(&c, c::TREASURY)?
    } else {
        empty(account(a, 3)?)?;
        let tomb = load::<TOMB_LEN>(account(a, 4)?, &TOMB_DISC)?;
        check_pda(account(a, 4)?, &[b"tomb", &[byte(&tomb, 40)?]])?;
        key(&tomb, 8)?
    };
    let mint = *account(a, 1)?.address();
    find(&[b"receipt", mint.as_ref()], account(a, 5)?)?;
    let creator = if account(a, 5)?.owned_by(&ID) {
        let r = load::<RECEIPT_LEN>(account(a, 5)?, &RECEIPT_DISC)?;
        check_pda(
            account(a, 5)?,
            &[b"receipt", mint.as_ref(), &[byte(&r, 40)?]],
        )?;
        key(&r, 8)?
    } else {
        // No enrolled identity: only unchanged, locked, exact Pump shares prove the payee.
        need(
            account(a, 5)?.owned_by(&SYSTEM)
                && account(a, 5)?.data_len() == 0
                && !account(a, 5)?.executable(),
            Error::NoReceipt,
        )?;
        find(&[b"token", mint.as_ref()], account(a, 7)?)?;
        need(
            account(a, 7)?.owned_by(&SYSTEM)
                && account(a, 7)?.data_len() == 0
                && !account(a, 7)?.executable(),
            Error::NoReceipt,
        )?;
        read_shares(
            account(a, 1)?,
            account(a, 8)?,
            account(a, 0)?.address(),
            &treasury,
        )?
        .0
    };
    let vb = [find(&[b"vault", mint.as_ref()], account(a, 0)?)?];
    money(account(a, 0)?, &[b"vault", mint.as_ref(), &vb])?;
    system(account(a, 6)?)?;
    creator_destination(account(a, 2)?, &mint, &treasury)?;
    need(account(a, 2)?.address() == &creator, Error::Unauthorized)?;
    // Pump CTO / termination cannot rewrite this immutable receipt or redirect the refund.
    let amount = account(a, 0)?.lamports().saturating_sub(rent(0)?);
    transfer(
        account(a, 0)?,
        account(a, 2)?,
        amount,
        &[
            Seed::from(b"vault"),
            Seed::from(mint.as_ref()),
            Seed::from(&vb),
        ],
    )?;
    emit(
        [63, 171, 52, 237, 126, 33, 208, 22],
        &[mint.as_ref(), creator.as_ref(), &amount.to_le_bytes()],
    )
}

fn close_config(a: &mut [AccountView], _d: &[u8]) -> ProgramResult {
    let c = config(account(a, 1)?)?;
    auth(&c, c::ADMIN, account(a, 0)?)?;
    need(byte(&c, c::CLOSING)? == 1, Error::NotWindingDown)?;
    need(u64_at(&c, c::OPEN)? == 0, Error::TokensOpen)?;
    system(account(a, 3)?)?;
    let bump = [find(&[b"tomb"], account(a, 2)?)?];
    init_account(
        account(a, 0)?,
        account(a, 2)?,
        TOMB_LEN,
        &[Seed::from(b"tomb"), Seed::from(&bump)],
    )?;
    let treasury = key(&c, c::TREASURY)?;
    let mut tomb = [0u8; TOMB_LEN];
    put(&mut tomb, 0, &TOMB_DISC)?;
    put(&mut tomb, 8, treasury.as_ref())?;
    put(&mut tomb, 40, &bump)?;
    save(
        a.get_mut(2).ok_or(ProgramError::NotEnoughAccountKeys)?,
        &tomb,
    )?;
    close_account(a, 1, 0)?;
    emit([4, 138, 208, 218, 204, 236, 118, 199], &[treasury.as_ref()])
}

fn read_shares(
    mint: &AccountView,
    sc: &AccountView,
    vault: &Address,
    treasury: &Address,
) -> Result<(Address, u16), ProgramError> {
    read_shares_data(mint, sc, vault, treasury).map_err(|e| {
        if e == ProgramError::InvalidAccountData {
            err(Error::BadPumpAccount)
        } else {
            e
        }
    })
}
fn read_shares_data(
    mint: &AccountView,
    sc: &AccountView,
    vault: &Address,
    treasury: &Address,
) -> Result<(Address, u16), ProgramError> {
    need(
        (mint.owned_by(&TOKEN) || mint.owned_by(&TOKEN_2022)) && !mint.executable(),
        Error::BadPumpAccount,
    )?;
    {
        let d = mint.try_borrow()?;
        need(d.len() >= 82 && byte(&d, 45)? == 1, Error::BadPumpAccount)?;
    }
    need(
        sc.owned_by(&PUMP_FEES)
            && !sc.executable()
            && sc.address()
                == &Address::find_program_address(
                    &[b"sharing-config", mint.address().as_ref()],
                    &PUMP_FEES,
                )
                .0,
        Error::BadPumpAccount,
    )?;
    let d = sc.try_borrow()?;
    need(
        d.len() >= 182 && rd::<8>(&d, 0)? == SC_DISC && key(&d, 11)? == *mint.address(),
        Error::BadPumpAccount,
    )?;
    need(
        byte(&d, 9)? == 2 && byte(&d, 10)? == 1 && byte(&d, 75)? == 1,
        Error::BadShares,
    )?;
    need(
        u32::from_le_bytes(rd(&d, 76)?) == 3 && vault != treasury,
        Error::BadShares,
    )?;
    let creator = key(&d, 43)?;
    let mut creator_bps = 0u16;
    let mut raid_bps = 0u16;
    let mut seen = 0u8;
    for i in 0..3 {
        let offset = 80 + i * 34;
        let address = key(&d, offset)?;
        let bps = u16::from_le_bytes(rd(&d, offset + 32)?);
        let bit = if address == *vault {
            need(
                (500..=8000).contains(&bps) && bps % 100 == 0,
                Error::BadShares,
            )?;
            raid_bps = bps;
            1
        } else if address == *treasury {
            need(bps == 1500, Error::BadShares)?;
            2
        } else {
            need(address == creator && creator != ZERO, Error::BadShares)?;
            creator_bps = bps;
            4
        };
        need(seen & bit == 0, Error::BadShares)?;
        seen |= bit;
    }
    need(
        seen == 7
            && creator_bps
                == 8500u16
                    .checked_sub(raid_bps)
                    .ok_or(err(Error::MathOverflow))?,
        Error::BadShares,
    )?;
    Ok((creator, raid_bps))
}
fn read_curve(mint: &AccountView, sc: &AccountView, bc: &AccountView) -> ProgramResult {
    need(
        bc.owned_by(&PUMP)
            && !bc.executable()
            && bc.address()
                == &Address::find_program_address(
                    &[b"bonding-curve", mint.address().as_ref()],
                    &PUMP,
                )
                .0,
        Error::BadPumpAccount,
    )?;
    let d = bc.try_borrow()?;
    need(
        d.len() >= 115 && rd::<8>(&d, 0)? == BC_DISC,
        Error::BadPumpAccount,
    )?;
    need(
        key(&d, 49)? == *sc.address() && byte(&d, 81)? == 0,
        Error::BadShares,
    )?;
    let quote = key(&d, 83)?;
    need(quote == ZERO || quote == WSOL, Error::BadShares)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn buckets_and_untrusted_reads() {
        assert_eq!(refill(0, 0, 30_000_000, 43200), Ok(7_500_000));
        assert_eq!(refill(29_000_000, 0, 30_000_000, 86400), Ok(7_500_000));
        assert_eq!(refill(123, 100, 30_000_000, 99), Ok(123));
        assert_eq!(refill(u64::MAX, 0, u64::MAX, i64::MAX), Ok(u64::MAX / 4));
        assert!(refill(0, i64::MIN, 1, i64::MAX).is_err());
        let mut d = [0u8; 16];
        put(&mut d, 8, &100i64.to_le_bytes()).expect("test timestamp");
        refill_at(&mut d, 0, 8, 86400, 90).expect("backward clock");
        refill_at(&mut d, 0, 8, 86400, 101).expect("forward clock");
        assert_eq!(u64_at(&d, 0), Ok(1));
        assert!(key(&[0; 31], 0).is_err());
        assert!(rd::<8>(&[0; 8], usize::MAX).is_err());
        assert!(byte(&[], 0).is_err());
        assert!(bps_valid(2550).is_err());
        assert!(bps_valid(500).is_ok());
        assert!(bps_valid(8000).is_ok());
        assert!(tier_cap(4).is_err());
    }
}

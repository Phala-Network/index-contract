use scale::{Decode, Encode, MaxEncodedLen};

/// Type used to encode the number of references an account has.
pub type RefCount = u32;
/// Index of a transaction in the chain.
pub type Index = u32;
/// Balance of an account.
pub type Balance = u128;

/// All balance information for an account.
#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, MaxEncodedLen)]
pub struct AccountData<Balance> {
    /// Non-reserved part of the balance. There may still be restrictions on this, but it is the
    /// total pool what may in principle be transferred, reserved and used for tipping.
    ///
    /// This is the only balance that matters in terms of most operations on tokens. It
    /// alone is used to determine the balance when in the contract execution environment.
    pub free: Balance,
    /// Balance which is reserved and may not be used at all.
    ///
    /// This can still get slashed, but gets slashed last of all.
    ///
    /// This balance is a 'reserve' balance that other subsystems use in order to set aside tokens
    /// that are still 'owned' by the account holder, but which are suspendable.
    /// This includes named reserve and unnamed reserve.
    pub reserved: Balance,
    /// The amount that `free` may not drop below when withdrawing for *anything except transaction
    /// fee payment*.
    pub misc_frozen: Balance,
    /// The amount that `free` may not drop below when withdrawing specifically for transaction
    /// fee payment.
    pub fee_frozen: Balance,
}

/// Information of an account.
#[derive(Clone, Eq, PartialEq, Default, Encode, Decode, MaxEncodedLen)]
pub struct AccountInfo<Index, AccountData> {
    /// The number of transactions this account has sent.
    pub nonce: Index,
    /// The number of other modules that currently depend on this account's existence. The account
    /// cannot be reaped until this is zero.
    pub consumers: RefCount,
    /// The number of other modules that allow this account to exist. The account may not be reaped
    /// until this and `sufficients` are both zero.
    pub providers: RefCount,
    /// The number of modules that allow this account to exist for their own purposes only. The
    /// account may not be reaped until this and `providers` are both zero.
    pub sufficients: RefCount,
    /// The additional data that belongs to this account. Used to store the balance(s) in a lot of
    /// chains.
    pub data: AccountData,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, MaxEncodedLen)]
pub enum ExistenceReason<Balance> {
    #[codec(index = 0)]
    Consumer,
    #[codec(index = 1)]
    Sufficient,
    #[codec(index = 2)]
    DepositHeld(Balance),
    #[codec(index = 3)]
    DepositRefunded,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, MaxEncodedLen)]
pub struct AssetAccount<Balance, DepositBalance, Extra> {
    /// The balance.
    pub(super) balance: Balance,
    /// Whether the account is frozen.
    pub(super) is_frozen: bool,
    /// The reason for the existence of the account.
    pub(super) reason: ExistenceReason<DepositBalance>,
    /// Additional "sidecar" data, in case some other pallet wants to use this storage item.
    pub(super) extra: Extra,
}

/// balance information for an account.
#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, MaxEncodedLen)]
pub struct OrmlTokenAccountData<Balance> {
    /// Non-reserved part of the balance. There may still be restrictions on
    /// this, but it is the total pool what may in principle be transferred,
    /// reserved.
    ///
    /// This is the only balance that matters in terms of most operations on
    /// tokens.
    pub free: Balance,
    /// Balance which is reserved and may not be used at all.
    ///
    /// This can still get slashed, but gets slashed last of all.
    ///
    /// This balance is a 'reserve' balance that other subsystems use in
    /// order to set aside tokens that are still 'owned' by the account
    /// holder, but which are suspendable.
    pub reserved: Balance,
    /// The amount that `free` may not drop below when withdrawing.
    pub frozen: Balance,
}

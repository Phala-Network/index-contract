use super::{AccountData, AccountInfo, AssetAccount, Balance, Index, OrmlTokenAccountData};
use crate::assets::*;
use crate::prelude::Error;
use alloc::string::String;
use alloc::{vec, vec::Vec};
use pink_extension::ResultExt;
use pink_subrpc::{
    get_next_nonce, get_ss58addr_version, get_storage,
    hasher::{Blake2_128, Twox64},
    storage::{storage_double_map_prefix, storage_map_prefix, storage_prefix},
    Ss58Codec,
};
use pink_web3::{
    api::{Eth, Namespace},
    contract::{Contract, Options},
    transports::{resolve_ready, PinkHttp},
    types::{Address, U256},
    Web3,
};
use scale::Encode;
use xcm::v1::MultiLocation;

#[derive(Clone, Debug, Default, PartialEq, Eq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum ChainType {
    #[default]
    Evm,
    Sub,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum ForeignAssetModule {
    #[default]
    PalletAsset,
    OrmlToken,
}

#[derive(Debug, Clone, Default, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct Chain {
    pub id: u32,
    pub name: String,
    pub endpoint: String,
    pub chain_type: ChainType,
    // Encoded native asset location for Sub-chains
    pub native_asset: Vec<u8>,
    pub foreign_asset: Option<ForeignAssetModule>,
    pub handler_contract: Vec<u8>,
}

impl Chain {
    pub fn is_native(&self, asset: &Vec<u8>) -> bool {
        match self.chain_type {
            ChainType::Evm => {
                // A little bit tricky here
                asset == &vec![0]
            }
            ChainType::Sub => asset == &self.native_asset,
        }
    }
}

/// Query on-chain `account` nonce
pub trait NonceFetcher {
    fn get_nonce(&self, account: Vec<u8>) -> core::result::Result<u64, Error>;
}
impl NonceFetcher for Chain {
    fn get_nonce(&self, account: Vec<u8>) -> core::result::Result<u64, Error> {
        Ok(match self.chain_type {
            ChainType::Evm => {
                let account20: [u8; 20] = account.try_into().map_err(|_| Error::InvalidAddress)?;
                let evm_account: Address = account20.into();
                let eth = Eth::new(PinkHttp::new(self.endpoint.clone()));
                let nonce = resolve_ready(eth.transaction_count(evm_account, None))
                    .map_err(|_| Error::FetchDataFailed)?;
                nonce.try_into().expect("Nonce onverflow")
            }
            ChainType::Sub => {
                let version = get_ss58addr_version(&self.name).map_err(|_| Error::Ss58)?;
                let public_key: [u8; 32] = account.try_into().map_err(|_| Error::InvalidAddress)?;
                let addr = public_key.to_ss58check_with_version(version.prefix());
                get_next_nonce(&self.endpoint, &addr).map_err(|_| Error::FetchDataFailed)?
            }
        })
    }
}

/// Query on-chain account balance of an asset
pub trait BalanceFetcher {
    fn get_balance(&self, asset: Vec<u8>, account: Vec<u8>) -> core::result::Result<u128, Error>;
}

impl BalanceFetcher for Chain {
    fn get_balance(&self, asset: Vec<u8>, account: Vec<u8>) -> core::result::Result<u128, Error> {
        match self.chain_type {
            ChainType::Evm => {
                let transport = PinkHttp::new(&self.endpoint);
                let account20: [u8; 20] = account.try_into().map_err(|_| Error::InvalidAddress)?;
                let evm_account: Address = account20.into();

                if self.is_native(&asset) {
                    let web3 = Web3::new(transport);
                    let balance = resolve_ready(web3.eth().balance(evm_account, None))
                        .log_err("Fetch data [evm native balance] failed")
                        .map_err(|_| Error::FetchDataFailed)?;
                    balance.try_into().map_err(|_| Error::BalanceOverflow)
                } else {
                    let eth = Eth::new(transport);
                    let asset_account20: [u8; 20] =
                        asset.try_into().map_err(|_| Error::InvalidAddress)?;
                    let token_address: Address = asset_account20.into();
                    let token = Contract::from_json(
                        eth,
                        token_address,
                        include_bytes!("../abis/erc20-abi.json"),
                    )
                    .expect("Bad abi data");
                    let balance: U256 = resolve_ready(token.query(
                        "balanceOf",
                        evm_account,
                        None,
                        Options::default(),
                        None,
                    ))
                    .log_err("Fetch data [evm erc20 balance] failed")
                    .map_err(|_| Error::FetchDataFailed)?;
                    balance.try_into().map_err(|_| Error::BalanceOverflow)
                }
            }
            ChainType::Sub => {
                let public_key: [u8; 32] = account.try_into().map_err(|_| Error::InvalidAddress)?;
                if self.is_native(&asset) {
                    if let Some(raw_storage) = get_storage(
                        &self.endpoint,
                        &storage_map_prefix::<Blake2_128>(
                            &storage_prefix("System", "Account")[..],
                            &public_key,
                        ),
                        None,
                    )
                    .log_err("Read storage [sub native balance] failed")
                    .map_err(|_| Error::FetchDataFailed)?
                    {
                        let account_info: AccountInfo<Index, AccountData<Balance>> =
                            scale::Decode::decode(&mut raw_storage.as_slice())
                                .log_err("Decode storage [sub native balance] failed")
                                .map_err(|_| Error::DecodeStorageFailed)?;
                        Ok(account_info.data.free)
                    } else {
                        Ok(0u128)
                    }
                } else {
                    let asset_location: MultiLocation =
                        scale::Decode::decode(&mut asset.as_slice())
                            .map_err(|_| Error::InvalidMultilocation)?;
                    match self.foreign_asset {
                        Some(ForeignAssetModule::PalletAsset) => {
                            let asset_id = Location2Assetid::new()
                                .get_assetid(self.name.clone(), &asset_location)
                                .ok_or(Error::AssetNotRecognized)?;
                            if let Some(raw_storage) = get_storage(
                                &self.endpoint,
                                &storage_double_map_prefix::<Blake2_128, Blake2_128>(
                                    &storage_prefix("Assets", "Account")[..],
                                    &asset_id.to_le_bytes(),
                                    &public_key,
                                ),
                                None,
                            )
                            .log_err(
                                "Read storage [sub foreign asset balance] from pallet-asset failed",
                            )
                            .map_err(|_| Error::FetchDataFailed)?
                            {
                                let account_info: AssetAccount<Balance, Balance, ()> =
                                    scale::Decode::decode(&mut raw_storage.as_slice())
                                    .log_err("Decode storage [sub foreign asset balance] from pallet-asset failed")
                                        .map_err(|_| Error::DecodeStorageFailed)?;
                                Ok(account_info.balance)
                            } else {
                                Ok(0u128)
                            }
                        }
                        Some(ForeignAssetModule::OrmlToken) => {
                            let currency_id = Location2Currencyid::new()
                                .get_currencyid(self.name.clone(), &asset_location)
                                .ok_or(Error::AssetNotRecognized)?;
                            if let Some(raw_storage) = get_storage(
                                &self.endpoint,
                                &storage_double_map_prefix::<Blake2_128, Twox64>(
                                    &storage_prefix("Tokens", "Accounts")[..],
                                    &public_key,
                                    &currency_id.encode(),
                                ),
                                None,
                            )
                            .log_err(
                                "Read storage [sub foreign asset balance] from orml-token failed",
                            )
                            .map_err(|_| Error::FetchDataFailed)?
                            {
                                let account_info: OrmlTokenAccountData<Balance> =
                                    scale::Decode::decode(&mut raw_storage.as_slice())
                                    .log_err("Decode storage [sub foreign asset balance] from orml-token failed")
                                        .map_err(|_| Error::DecodeStorageFailed)?;
                                Ok(account_info.free)
                            } else {
                                Ok(0u128)
                            }
                        }
                        None => Err(Error::Unimplemented),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotenv::dotenv;
    use hex_literal::hex;
    use scale::Encode;
    use sp_runtime::{traits::ConstU32, WeakBoundedVec};
    use xcm::v1::{prelude::*, MultiLocation};

    #[test]
    fn test_get_evm_account_nonce() {
        dotenv().ok();
        pink_extension_runtime::mock_ext::mock_all_ext();

        let goerli = Chain {
            id: 1,
            name: String::from("Goerli"),
            endpoint: String::from(
                "https://eth-goerli.g.alchemy.com/v2/lLqSMX_1unN9Xrdy_BB9LLZRgbrXwZv2",
            ),
            chain_type: ChainType::Evm,
            native_asset: vec![0],
            foreign_asset: None,
            handler_contract: hex!("056C0E37d026f9639313C281250cA932C9dbe921").into(),
        };
        assert_eq!(
            goerli
                .get_nonce(hex!("0E275F8839b788B2674935AD97C01cF73A9E8c41").into())
                .unwrap(),
            2
        );
    }

    #[test]
    fn test_get_sub_account_nonce() {
        dotenv().ok();
        pink_extension_runtime::mock_ext::mock_all_ext();

        let khala = Chain {
            id: 1,
            name: String::from("Khala"),
            endpoint: String::from("https://khala.api.onfinality.io:443/public-ws"),
            chain_type: ChainType::Sub,
            native_asset: MultiLocation::new(1, X1(Parachain(2004))).encode(),
            foreign_asset: Some(ForeignAssetModule::PalletAsset),
            handler_contract: hex!("056C0E37d026f9639313C281250cA932C9dbe921").into(),
        };
        assert_eq!(
            khala
                .get_nonce(
                    hex!("92436be04f9dc677f9f51b092161b6e5ba00163ad6328fb2c920fcb30b6c7362").into()
                )
                .unwrap(),
            2
        );
    }

    #[test]
    fn test_get_evm_account_balance() {
        dotenv().ok();
        pink_extension_runtime::mock_ext::mock_all_ext();

        let goerli = Chain {
            id: 1,
            name: String::from("Goerli"),
            endpoint: String::from(
                "https://eth-goerli.g.alchemy.com/v2/lLqSMX_1unN9Xrdy_BB9LLZRgbrXwZv2",
            ),
            chain_type: ChainType::Evm,
            native_asset: vec![0],
            foreign_asset: None,
            handler_contract: hex!("056C0E37d026f9639313C281250cA932C9dbe921").into(),
        };
        // Get native asset balance
        assert_eq!(
            goerli
                .get_balance(
                    hex!("00").into(),
                    hex!("0E275F8839b788B2674935AD97C01cF73A9E8c41").into()
                )
                .unwrap(),
            6_850_126_116_190_000u128
        );
        // Get GPHA balance
        assert_eq!(
            goerli
                .get_balance(
                    hex!("B376b0Ee6d8202721838e76376e81eEc0e2FE864").into(),
                    hex!("0E275F8839b788B2674935AD97C01cF73A9E8c41").into()
                )
                .unwrap(),
            5_000_000_000_000_000_000u128
        );
    }

    #[test]
    fn test_get_sub_account_balance() {
        dotenv().ok();
        pink_extension_runtime::mock_ext::mock_all_ext();

        let account32 = hex!("92436be04f9dc677f9f51b092161b6e5ba00163ad6328fb2c920fcb30b6c7362");
        let khala = Chain {
            id: 1,
            name: String::from("Khala"),
            endpoint: String::from("https://khala.api.onfinality.io:443/public-ws"),
            chain_type: ChainType::Sub,
            native_asset: MultiLocation::new(1, X1(Parachain(2004))).encode(),
            foreign_asset: Some(ForeignAssetModule::PalletAsset),
            handler_contract: hex!("056C0E37d026f9639313C281250cA932C9dbe921").into(),
        };
        let karura = Chain {
            id: 2,
            name: String::from("Karura"),
            endpoint: String::from("https://karura-rpc.dwellir.com"),
            chain_type: ChainType::Sub,
            native_asset: MultiLocation::new(
                1,
                X2(
                    Parachain(2000),
                    GeneralKey(WeakBoundedVec::<u8, ConstU32<32>>::force_from(
                        vec![0x00, 0x80],
                        None,
                    )),
                ),
            )
            .encode(),
            foreign_asset: Some(ForeignAssetModule::OrmlToken),
            handler_contract: hex!("056C0E37d026f9639313C281250cA932C9dbe921").into(),
        };
        // Get native asset (PHA on Khala)
        assert_eq!(
            khala
                .get_balance(khala.native_asset.clone(), account32.into())
                .unwrap(),
            96_879_782_174u128
        );
        // Get foreign asset managed by pallet-assets (KAR on Khala)
        assert_eq!(
            khala
                .get_balance(karura.native_asset.clone(), account32.into())
                .unwrap(),
            40_000_000_000u128
        );
        // Get foreign asset managed by orml tokens (PHA on Karura)
        assert_eq!(
            karura
                .get_balance(khala.native_asset, account32.into())
                .unwrap(),
            80_000_000_000u128
        );
    }
}

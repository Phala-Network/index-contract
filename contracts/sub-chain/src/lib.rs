#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

use pink_extension as pink;

#[pink::contract(env = PinkEnvironment)]
#[pink(inner=ink_lang::contract)]
mod sub_chain {
    use super::pink;
    use alloc::vec;
    use alloc::{string::String, vec::Vec};
    use ink_lang as ink;
    use phala_pallet_common::WrapSlice;
    use pink::{http_get, PinkEnvironment};
    use pink_web3::api::{Eth, Namespace};
    use pink_web3::contract::{Contract, Options};
    use pink_web3::transports::{resolve_ready, PinkHttp};
    use pink_web3::types::{Address, Res, H256};
    use scale::{Decode, Encode};
    use traits::ensure;
    use traits::registry::{
        AssetInfo, AssetsRegisry, BalanceFetcher, ChainInspector, ChainType, Error as RegistryError,
    };
    use xcm::latest::{prelude::*, Fungibility::Fungible, MultiAsset, MultiLocation};

    #[ink(storage)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct EvmChain {
        admin: AccountId,
        /// The chain name
        chain: Vec<u8>,
        /// Type of chain
        chain_type: ChainType,
        /// Network id of chain
        network_id: u8,
        /// The registered assets list
        assets: Vec<AssetInfo>,
        /// Native asset of chain
        native: Option<AssetInfo>,
        /// Stable asset of chain
        stable: Option<AssetInfo>,
        /// RPC endpoint of chain
        endpoint: Vec<u8>,
    }

    /// Event emitted when native asset set.
    #[ink(event)]
    pub struct NativeSet {
        #[ink(topic)]
        chain: Vec<u8>,
        #[ink(topic)]
        asset: Option<AssetInfo>,
    }

    /// Event emitted when stable asset set.
    #[ink(event)]
    pub struct StableSet {
        #[ink(topic)]
        chain: Vec<u8>,
        #[ink(topic)]
        asset: Option<AssetInfo>,
    }

    /// Event emitted when RPC endpoint asset set.
    #[ink(event)]
    pub struct EndpointSet {
        #[ink(topic)]
        chain: Vec<u8>,
        #[ink(topic)]
        endpoint: Vec<u8>,
    }

    /// Event emitted when asset registered.
    #[ink(event)]
    pub struct Registered {
        #[ink(topic)]
        chain: Vec<u8>,
        #[ink(topic)]
        asset: Option<AssetInfo>,
    }

    /// Event emitted when asset unregistered.
    #[ink(event)]
    pub struct Unregistered {
        #[ink(topic)]
        chain: Vec<u8>,
        #[ink(topic)]
        asset: Option<AssetInfo>,
    }

    pub type Result<T> = core::result::Result<T, RegistryError>;
    
    impl SubChain {
        #[ink(constructor)]
        /// Create an Substrate chain entity
        pub fn new(chain: Vec<u8>, network_id: u8, endpoint: Vec<u8>) -> Self {
            SubChain {
                admin: Self::env().caller(),
                chain,
                network_id,
                chain_type: ChainType::Sub,
                assets: vec![],
                native: None,
                stable: None,
                endpoint,
            }
        }

        /// Set native asset
        /// Authorized method, only the contract owner can do
        #[ink(message)]
        pub fn set_native(&mut self, asset: AssetInfo) -> Result<()> {
            self.esure_admin()?;
            self.native = Some(asset.clone());
            Self::env().emit_event(NativeSet {
                chain: self.chain.clone(),
                asset: Some(asset),
            });
            Ok(())
        }

        /// Set native asset
        /// Authorized method, only the contract owner can do
        #[ink(message)]
        pub fn set_stable(&mut self, asset: AssetInfo) -> Result<()> {
            self.esure_admin()?;
            self.stable = Some(asset.clone());
            Self::env().emit_event(StableSet {
                chain: self.chain.clone(),
                asset: Some(asset),
            });
            Ok(())
        }

        /// Set RPC endpoint
        /// Authorized method, only the contract owner can do
        #[ink(message)]
        pub fn set_endpoint(&mut self, endpoint: Vec<u8>) -> Result<()> {
            self.esure_admin()?;
            self.endpoint = endpoint.clone();
            Self::env().emit_event(EndpointSet {
                chain: self.chain.clone(),
                endpoint,
            });
            Ok(())
        }

        /// Returns error if caller is not admin
        fn esure_admin(&self) -> Result<()> {
            let caller = self.env().caller();
            if self.admin != caller {
                return Err(RegistryError::BadOrigin);
            }
            Ok(())
        }

        /// An asset id represented by MultiLocation like:
        /// (1, X4(Parachain(phala_id), GeneralKey(“phat"), GeneralKey(cluster_id), GeneralKey(erc20_address)))
        fn extract_token(&self, asset: &AssetId) -> Option<Address> {
            match asset {
                Concrete(location) => {
                    match (location.parents, &location.interior) {
                        (
                            1,
                            Junctions::X4(
                                Parachain(_id),
                                GeneralKey(_phat_key),
                                GeneralKey(_cluster_id),
                                GeneralKey(erc20_address),
                            ),
                        ) => {
                            // TODO.wf verify arguments
                            if erc20_address.len() != 20 {
                                return None;
                            };
                            let address: Address = Address::from_slice(&erc20_address);
                            Some(address)
                        }
                        _ => None,
                    }
                }
                _ => None,
            }
        }

        /// An account location represented by MultiLocation like:
        /// (1, X4(Parachain(phala_id), GeneralKey(“phat"), GeneralKey(cluster_id), GeneralKey(account_address)))
        fn extract_account(&self, location: &MultiLocation) -> Option<Address> {
            match (location.parents, &location.interior) {
                (
                    1,
                    Junctions::X4(
                        Parachain(_id),
                        GeneralKey(_phat_key),
                        GeneralKey(_cluster_id),
                        GeneralKey(account_address),
                    ),
                ) => {
                    // TODO.wf verify arguments
                    if account_address.len() != 20 {
                        return None;
                    };
                    let address: Address = Address::from_slice(&account_address);
                    Some(address)
                }
                _ => None,
            }
        }
    }

    impl ChainInspector for SubChain {
        /// Return admin of the chain
        #[ink(message)]
        fn owner(&self) -> AccountId {
            self.admin
        }

        /// Return name of the chain
        #[ink(message)]
        fn chain_name(&self) -> Vec<u8> {
            self.chain.clone()
        }

        /// Return set native asset of the chain
        #[ink(message)]
        fn chain_type(&self) -> ChainType {
            self.chain_type.clone()
        }

        /// Return set native asset of the chain
        #[ink(message)]
        fn native_asset(&self) -> Option<AssetInfo> {
            self.native.clone()
        }

        /// Return set stable asset of the chain
        #[ink(message)]
        fn stable_asset(&self) -> Option<AssetInfo> {
            self.stable.clone()
        }

        /// Return RPC endpoint of the chain
        #[ink(message)]
        fn endpoint(&self) -> Vec<u8> {
            self.endpoint.clone()
        }
    }

    impl BalanceFetcher for SubChain {
        #[ink(message)]
        fn balance_of(&self, asset: AssetId, account: MultiLocation) -> Result<u128> {
            Ok(0)
        }
    }

    impl AssetsRegisry for SubChain {
        /// Register the asset
        /// Authorized method, only the contract owner can do
        #[ink(message)]
        fn register(&mut self, asset: AssetInfo) -> Result<()> {
            self.esure_admin()?;

            ensure!(
                self.assets
                    .iter()
                    .position(|a| a.location == asset.location)
                    .is_none(),
                RegistryError::AssetAlreadyRegistered
            );
            self.assets.push(asset.clone());

            Self::env().emit_event(Registered {
                chain: self.chain.clone(),
                asset: Some(asset),
            });
            Ok(())
        }

        /// Unregister the asset
        /// Authorized method, only the contract owner can do
        #[ink(message)]
        fn unregister(&mut self, asset: AssetInfo) -> Result<()> {
            self.esure_admin()?;

            let index = self
                .assets
                .iter()
                .position(|a| a.location == asset.location)
                .ok_or(RegistryError::AssetNotFound)?;
            self.assets.remove(index);

            Self::env().emit_event(Unregistered {
                chain: self.chain.clone(),
                asset: Some(asset),
            });
            Ok(())
        }

        /// Return all registerd assets
        #[ink(message)]
        fn registered_assets(&self) -> Vec<AssetInfo> {
            self.assets.clone()
        }

        #[ink(message)]
        fn lookup_by_name(&self, name: Vec<u8>) -> Option<AssetInfo> {
            self.assets
                .iter()
                .position(|a| a.name == name)
                .and_then(|idx| Some(self.assets[idx].clone()))
        }

        #[ink(message)]
        fn lookup_by_symbol(&self, symbol: Vec<u8>) -> Option<AssetInfo> {
            self.assets
                .iter()
                .position(|a| a.symbol == symbol)
                .and_then(|idx| Some(self.assets[idx].clone()))
        }

        #[ink(message)]
        fn lookup_by_location(&self, location: Vec<u8>) -> Option<AssetInfo> {
            self.assets
                .iter()
                .position(|a| a.location == location)
                .and_then(|idx| Some(self.assets[idx].clone()))
        }
    }

    #[cfg(test)]
    mod test {
        use super::*;
        use dotenv::dotenv;
        use ink_lang as ink;

    }
}
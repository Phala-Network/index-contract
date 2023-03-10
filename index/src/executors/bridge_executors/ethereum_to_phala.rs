use pink_subrpc as subrpc;

use crate::traits::{common::Error, executor::BridgeExecutor};
use crate::transactors::ChainBridgeEvmClient;
use crate::utils::ToArray;
use alloc::vec::Vec;
use pink_web3::{
    api::{Eth, Namespace},
    contract::Contract,
    keys::pink::KeyPair,
    transports::PinkHttp,
    types::{Address, U256},
};
use scale::Encode;
use subrpc::ExtraParam;
use xcm::v1::{prelude::*, Junction, Junctions, MultiLocation};

#[derive(Clone)]
pub struct ChainBridgeEthereum2Phala {
    // ((asset_contract_address, dest_chain), resource_id)
    assets: Vec<(Address, [u8; 32])>,
    bridge_contract: ChainBridgeEvmClient,
}

#[allow(dead_code)]
impl ChainBridgeEthereum2Phala {
    pub fn new(
        rpc: &str,
        dest_chain_id: u8,
        bridge_address: Address,
        assets: Vec<(Address, [u8; 32])>,
    ) -> Self {
        let eth = Eth::new(PinkHttp::new(rpc));
        let bridge_contract = ChainBridgeEvmClient {
            dest_chain_id,
            contract: Contract::from_json(
                eth,
                bridge_address,
                include_bytes!("../../abis/chainbridge-abi.json"),
            )
            .expect("Bad abi data"),
        };

        Self {
            assets,
            bridge_contract,
        }
    }

    fn lookup_rid(&self, addr: Address) -> Option<[u8; 32]> {
        self.assets
            .iter()
            .position(|a| a.0 == addr)
            .map(|idx| self.assets[idx].1)
    }
}

#[allow(dead_code)]
impl BridgeExecutor for ChainBridgeEthereum2Phala {
    fn transfer(
        &self,
        signer: [u8; 32],
        asset: Vec<u8>,
        recipient: Vec<u8>,
        amount: u128,
        extra: ExtraParam,
    ) -> core::result::Result<Vec<u8>, Error> {
        let signer = KeyPair::from(signer);
        let recipient: [u8; 32] = recipient.try_into().map_err(|_| Error::InvalidAddress)?;
        let dest = MultiLocation::new(
            0,
            Junctions::X1(Junction::AccountId32 {
                network: NetworkId::Any,
                id: recipient,
            }),
        );
        let asset: [u8; 20] = asset.to_array();
        let rid = self
            .lookup_rid(asset.into())
            .ok_or(Error::InvalidMultilocation)?;
        let tx_id = self.bridge_contract.deposit(
            signer,
            rid.into(),
            U256::from(amount),
            dest.encode(),
            extra.nonce,
        )?;
        Ok(tx_id.as_bytes().to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::ChainBridgeEthereum2Phala;
    use crate::constants::CHAINBRIDGE_ID_KHALA;
    use crate::traits::executor::BridgeExecutor;
    use crate::utils::ToArray;
    use pink_subrpc::ExtraParam;
    use pink_web3::ethabi::Address;

    #[test]
    #[ignore]
    fn chainbridge_pha_ethereum2phala_works() {
        pink_extension_runtime::mock_ext::mock_all_ext();

        let secret_key = std::env::vars().find(|x| x.0 == "SECRET_KEY");
        let secret_key = secret_key.unwrap().1;
        let secret_bytes = hex::decode(secret_key).unwrap();
        let signer: [u8; 32] = secret_bytes.to_array();

        let chainbridge_on_goerli: Address =
            hex_literal::hex!("056C0E37d026f9639313C281250cA932C9dbe921").into();
        let pha_on_goerli: Address =
            hex_literal::hex!("B376b0Ee6d8202721838e76376e81eEc0e2FE864").into();
        let chainbridge = ChainBridgeEthereum2Phala::new(
            "https://eth-goerli.g.alchemy.com/v2/lLqSMX_1unN9Xrdy_BB9LLZRgbrXwZv2",
            CHAINBRIDGE_ID_KHALA,
            chainbridge_on_goerli,
            vec![(
                pha_on_goerli,
                // PHA ChainBridge resource id on Thala (Test runtime of Phala Network)
                hex_literal::hex!(
                    "00e6dfb61a2fb903df487c401663825643bb825d41695e63df8af6162ab145a6"
                ),
            )],
        );
        let recipient: Vec<u8> =
            hex_literal::hex!("7804e66ec9eea3d8daf6273ffbe0a8af25a8879cf43f14d0ebbb30941f578242")
                .into();

        let tx_id = chainbridge.transfer(
            signer,
            pha_on_goerli.as_bytes().to_vec(),
            recipient,
            // 1 PHA (decimals is 18 on Goerli)
            1_000_000_000_000_000_000u128,
            ExtraParam::default(),
        );
        println!(
            "ChainBridgeEthereum2Phala: send tx {:?}",
            hex::encode(tx_id.unwrap())
        );
    }
}

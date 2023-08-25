use alloc::{collections::BTreeMap, vec, vec::Vec};
use core::str::FromStr;
use hex_literal::hex;
use pink_web3::{
    api::{Eth, Namespace},
    contract::{tokens::Tokenize, Contract, Options},
    ethabi::{Address, Uint},
    transports::{resolve_ready, PinkHttp},
    types::U256,
};
use scale::Encode;
use xcm::v3::{
    Junction::{AccountId32, Parachain},
    Junctions::{X1, X2},
    MultiLocation,
};

use crate::call::{Call, CallBuilder, CallParams, EvmCall};
use crate::step::Step;

#[derive(Clone)]
pub struct EvmSygmaBridge {
    eth: Eth<PinkHttp>,
    contract: Contract<PinkHttp>,
    erc20_handler_address: Address,
    fee_handler_address: Address,
    from_domain_id: u8,
    to_domain_id: u8,
    maybe_parachain_id: Option<u32>,
    resource_id_map: BTreeMap<Address, [u8; 32]>,
}

impl EvmSygmaBridge {
    pub fn new(
        rpc: &str,
        contract_address: Address,
        erc20_handler_address: Address,
        fee_handler_address: Address,
        from_domain_id: u8,
        to_domain_id: u8,
        maybe_parachain_id: Option<u32>,
    ) -> Self {
        let eth = Eth::new(PinkHttp::new(rpc));
        let contract = Contract::from_json(
            eth.clone(),
            contract_address,
            include_bytes!("../../abi/SygmaBridge.json"),
        )
        .expect("Bad abi data");

        let mut resource_id_map = BTreeMap::new();
        // Goerli GPHA
        resource_id_map.insert(
            Address::from_str("B376b0Ee6d8202721838e76376e81eEc0e2FE864").unwrap(),
            hex!("0000000000000000000000000000000000000000000000000000000000001000"),
        );
        // Ethereum PHA
        resource_id_map.insert(
            Address::from_str("6c5bA91642F10282b576d91922Ae6448C9d52f4E").unwrap(),
            hex!("0000000000000000000000000000000000000000000000000000000000000001"),
        );

        Self {
            eth,
            contract,
            erc20_handler_address,
            fee_handler_address,
            from_domain_id,
            to_domain_id,
            maybe_parachain_id,
            resource_id_map,
        }
    }
}

impl CallBuilder for EvmSygmaBridge {
    fn build_call(&self, step: Step) -> Result<Vec<Call>, &'static str> {
        let sender = Address::from_slice(&step.sender.ok_or("MissingSender")?);
        let spend_asset = Address::from_slice(&step.spend_asset);
        let resource_id = self
            .resource_id_map
            .get(&spend_asset)
            .ok_or("NoResourceId")?
            .clone();
        let receive_asset = Address::from_slice(&step.receive_asset);
        let spend_amount = U256::from(step.spend_amount.ok_or("MissingSpendAmount")?);
        let mut recipient = step.recipient.ok_or("MissingRecipient")?;
        if recipient.len() == 32 {
            let account_id = AccountId32 {
                network: None,
                id: recipient.try_into().unwrap(),
            };
            let rec = match self.maybe_parachain_id {
                Some(parachain_id) => {
                    MultiLocation::new(1, X2(Parachain(parachain_id), account_id))
                }
                None => MultiLocation::new(0, X1(account_id)),
            };
            recipient = rec.encode()
        }
        let mut deposit_data: Vec<u8> = vec![];
        let token_stats: [u8; 32] = spend_amount.into();
        let mut recipient_len: [u8; 32] = [0; 32];
        recipient_len[24..].copy_from_slice(&recipient.len().to_be_bytes());
        deposit_data.extend(token_stats);
        deposit_data.extend(recipient_len);
        deposit_data.extend(recipient);

        let fee_handler = Contract::from_json(
            self.eth.clone(),
            self.fee_handler_address,
            include_bytes!("../../abi/SygmaBasicFeeHandler.json"),
        )
        .expect("Bad abi data");

        let fee: (Uint, Address) = resolve_ready(fee_handler.query(
            "calculateFee",
            (
                sender,
                self.from_domain_id,
                self.to_domain_id,
                resource_id,
                hex!("").to_vec(),
                hex!("").to_vec(),
            ),
            None,
            Options::default(),
            None,
        ))
        .unwrap();

        let mut fee_data = vec![0u8; 32];
        fee.0.to_big_endian(&mut fee_data);
        let nonzero_index = fee_data
            .iter()
            .position(|&x| x != 0)
            .unwrap_or(fee_data.len() - 1);
        let fee_data: Vec<u8> = fee_data[nonzero_index..].to_vec();

        let bridge_params = (self.to_domain_id, resource_id, deposit_data, fee_data);

        let bridge_func = self
            .contract
            .abi()
            .function("deposit")
            .map_err(|_| "NoFunctionFound")?;
        let bridge_calldata = bridge_func
            .encode_input(&bridge_params.into_tokens())
            .map_err(|_| "EncodeParamError")?;

        let token = Contract::from_json(
            self.eth.clone(),
            spend_asset,
            include_bytes!("../../abi/erc20.json"),
        )
        .expect("Bad abi data");
        let approve_params = (self.erc20_handler_address, spend_amount);
        let approve_func = token
            .abi()
            .function("approve")
            .map_err(|_| "NoFunctionFound")?;
        let approve_calldata = approve_func
            .encode_input(&approve_params.into_tokens())
            .map_err(|_| "EncodeParamError")?;

        Ok(vec![
            Call {
                params: CallParams::Evm(EvmCall {
                    target: spend_asset,
                    calldata: approve_calldata,
                    value: U256::from(0),

                    need_settle: false,
                    update_offset: U256::from(36),
                    update_len: U256::from(32),
                    spend_asset,
                    spend_amount,
                    receive_asset,
                }),
                input_call: None,
                call_index: None,
            },
            Call {
                params: CallParams::Evm(EvmCall {
                    target: self.contract.address(),
                    calldata: bridge_calldata,
                    value: U256::from(0),

                    need_settle: true,
                    update_offset: U256::from(36),
                    update_len: U256::from(32),
                    spend_asset,
                    spend_amount,
                    receive_asset,
                }),
                input_call: None,
                call_index: None,
            },
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp_runtime::AccountId32;

    #[test]
    fn test_pha_from_goerli_to_rhala() {
        pink_extension_runtime::mock_ext::mock_all_ext();

        let rpc = "https://rpc.ankr.com/eth_goerli";
        let sygma_bridge = EvmSygmaBridge::new(
            rpc,
            Address::from_str("c26335a9f16398b5fDA4bC05b62C1429D8a4d755").unwrap(),
            Address::from_str("7Ed4B14a82B2F2C4DfB13DC4Eac00205EDEff6C2").unwrap(),
            Address::from_str("e6CE0ea4eC6ECbdC23eEF9f4fB165aCc979C56b5").unwrap(),
            1,
            3,
            None,
        );

        let recipient: [u8; 32] =
            AccountId32::from_str("412WzkzTZRXWvb5pwZfew4TCB6z2nTS4t4FhY3LJgd8XMoQ2")
                .unwrap()
                .into();

        let calls = sygma_bridge
            .build_call(Step {
                exe_type: String::from(""),
                exe: String::from(""),
                source_chain: String::from("Rhala"),
                dest_chain: String::from("Goerli"),
                spend_asset: hex!("B376b0Ee6d8202721838e76376e81eEc0e2FE864").to_vec(),
                receive_asset: hex!("B376b0Ee6d8202721838e76376e81eEc0e2FE864").to_vec(),
                sender: Some(hex!("53e4C6611D3C92232bCBdd20D1073ce892D34594").to_vec()),
                recipient: Some(recipient.to_vec()),
                spend_amount: Some(20_000_000_000_000_000_000 as u128),
                origin_balance: None,
                nonce: None,
            })
            .unwrap();

        match &calls[1].params {
            CallParams::Evm(evm_call) => {
                // https://goerli.etherscan.io/tx/0x42261d70e9849a30dd878c53d185136972deea0420322978fafe6313682da804
                let encoded_data = hex::encode(&evm_call.calldata);
                assert_eq!(encoded_data, String::from("73c45c9800000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000001200000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000001158e460913d000000000000000000000000000000000000000000000000000000000000000000024000101001218d75948e2983aa273f5dff9dced1935ce4704492ba6ea30f6d33a35010d3b000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007038d7ea4c6800000000000000000000000000000000000000000000000000000"));
            }
            _ => {
                println!("Not an EvmCall");
            }
        }
    }
}

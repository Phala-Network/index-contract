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
    fee_amount: u128,
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
        fee_amount: u128,
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
            fee_amount,
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
                    receive_asset: spend_asset,
                }),
                input_call: None,
                call_index: None,
            },
            Call {
                params: CallParams::Evm(EvmCall {
                    target: self.contract.address(),
                    calldata: bridge_calldata,
                    value: U256::from(self.fee_amount),

                    need_settle: false,
                    update_offset: U256::from(164),
                    update_len: U256::from(32),
                    spend_asset,
                    spend_amount,
                    receive_asset: spend_asset,
                }),
                input_call: None,
                call_index: None,
            },
        ])
    }
}

#[cfg(test)]
mod tests {
    // use crate::utils::ToArray;

    use super::*;
    use sp_runtime::AccountId32;
    // use pink_web3::keys::pink::KeyPair;
    use pink_web3::types::H160;

    #[test]
    fn test_pha_from_goerli_to_rhala() {
        pink_extension_runtime::mock_ext::mock_all_ext();

        // let secret_key = std::env::vars().find(|x| x.0 == "SECRET_KEY");
        // let secret_key = secret_key.unwrap().1;
        // let secret_bytes = hex::decode(secret_key).unwrap();
        // let _signer: [u8; 32] = secret_bytes.to_array();

        let rpc = "https://rpc.ankr.com/eth_goerli";

        // Handler on Goerli
        let handler_address: H160 =
            H160::from_slice(&hex::decode("0b45A95d0A8736b4e7FB5B9f11b0D8F5Ac078860").unwrap());
        let transport = Eth::new(PinkHttp::new(rpc.clone()));
        let handler = Contract::from_json(
            transport,
            handler_address,
            include_bytes!("../../abi/handler.json"),
        )
        .unwrap();
        let sygma_bridge = EvmSygmaBridge::new(
            rpc,
            Address::from_str("c26335a9f16398b5fDA4bC05b62C1429D8a4d755").unwrap(),
            Address::from_str("7Ed4B14a82B2F2C4DfB13DC4Eac00205EDEff6C2").unwrap(),
            Address::from_str("e6CE0ea4eC6ECbdC23eEF9f4fB165aCc979C56b5").unwrap(),
            // 0.001 ETH on Goerli
            1_000_000_000_000_000u128,
            1,
            3,
            None,
        );

        let recipient: [u8; 32] =
            AccountId32::from_str("412WzkzTZRXWvb5pwZfew4TCB6z2nTS4t4FhY3LJgd8XMoQ2")
                .unwrap()
                .into();

        let mut calls = sygma_bridge
            .build_call(Step {
                exe_type: String::from(""),
                exe: String::from(""),
                source_chain: String::from("Goerli"),
                dest_chain: String::from("Rhaala"),
                spend_asset: hex::decode("B376b0Ee6d8202721838e76376e81eEc0e2FE864").unwrap(),
                // MuliLocation: (0, Here)
                receive_asset: hex::decode("0000").unwrap(),
                sender: Some(hex::decode("53e4C6611D3C92232bCBdd20D1073ce892D34594").unwrap()),
                recipient: Some(recipient.to_vec()),
                spend_amount: Some(20_000_000_000_000_000_000 as u128),
                origin_balance: None,
                nonce: None,
            })
            .unwrap();

        // Apply index mannually
        calls[0].input_call = Some(0);
        calls[0].call_index = Some(0);
        calls[1].input_call = Some(0);
        calls[1].call_index = Some(1);

        // Make sure handler address hold enough spend asset and native asset (e.g. ETH).
        // Because handler is the account who spend and pay fee on behalf

        // Estiamte gas before submission
        let _gas = resolve_ready(handler.estimate_gas(
            "batchCall",
            calls.clone(),
            // Worker address
            Address::from_slice(&hex::decode("bf526928373748b00763875448ee905367d97f96").unwrap()),
            Options::with(|opt| {
                // 0.001 ETH
                opt.value = Some(U256::from(1_000_000_000_000_000u128))
            }),
        ))
        .map_err(|e| {
            println!("Failed to estimated step gas cost with error: {:?}", e);
            "FailedToEstimateGas"
        })
        .unwrap();

        // Tested on Georli:
        // Goerli: https://goerli.etherscan.io/tx/0x0157f0409e8a8769da5c2b58a8cc8f24e784fd0e2a52f2b74f9c1f7b9c5f60b8
        // Rhala:

        // Uncomment if wanna send it to blockchain
        // let _tx_id: primitive_types::H256 = resolve_ready(handler.signed_call(
        //     "batchCall",
        //     calls,
        //     Options::with(|opt| {
        //         opt.gas = Some(gas);
        //         // 0.001 ETH
        //         opt.value = Some(U256::from(1_000_000_000_000_000u128))
        //     }),
        //     KeyPair::from(signer),
        // ))
        // .map_err(|e| {
        //     println!(
        //         "Failed to submit step execution tx with error: {:?}",
        //         e
        //     );
        //     "FailedToSubmitTransaction"
        // }).unwrap();

        // match &calls[1].params {
        //     CallParams::Evm(evm_call) => {
        //         // https://goerli.etherscan.io/tx/0x42261d70e9849a30dd878c53d185136972deea0420322978fafe6313682da804
        //         let encoded_data = hex::encode(&evm_call.calldata);
        //         assert_eq!(encoded_data, String::from("73c45c9800000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000001200000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000001158e460913d000000000000000000000000000000000000000000000000000000000000000000024000101001218d75948e2983aa273f5dff9dced1935ce4704492ba6ea30f6d33a35010d3b000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000007038d7ea4c6800000000000000000000000000000000000000000000000000000"));
        //     }
        //     _ => {
        //         println!("Not an EvmCall");
        //     }
        // }
    }
}

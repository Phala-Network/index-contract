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

use crate::step::Step;
use crate::{
    call::{Call, CallBuilder, CallParams, EvmCall},
    utils::ToArray,
};

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

#[allow(clippy::too_many_arguments)]
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
    fn build_call(&self, step: Step) -> Result<Call, &'static str> {
        let sender = Address::from_slice(&step.sender.ok_or("MissingSender")?);
        let spend_asset = Address::from_slice(&step.spend_asset);
        let resource_id = *self
            .resource_id_map
            .get(&spend_asset)
            .ok_or("NoResourceId")?;
        let spend_amount = U256::from(step.spend_amount.ok_or("MissingSpendAmount")?);
        let mut recipient = step.recipient;
        if recipient.len() == 32 {
            let account_id = AccountId32 {
                network: None,
                id: recipient.to_array(),
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
        deposit_data.extend(token_stats);
        deposit_data.extend_from_slice(&{
            let mut res = Vec::new();
            for b in U256::from(recipient.len()).0.iter().rev() {
                let bytes = b.to_be_bytes();
                res.extend(bytes);
            }
            res
        });
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

        Ok(Call {
            params: CallParams::Evm(EvmCall {
                target: self.contract.address(),
                calldata: bridge_calldata,
                value: U256::from(self.fee_amount),
                spender: self.erc20_handler_address,
                need_settle: false,
                update_offset: U256::from(164),
                update_len: U256::from(32),
                spend_asset,
                spend_amount,
                receive_asset: spend_asset,
            }),
            input_call: None,
            call_index: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::ToArray;

    use super::*;
    use pink_web3::keys::pink::KeyPair;
    use pink_web3::types::H160;

    #[test]
    #[ignore]
    fn test_pha_from_goerli_to_rhala() {
        pink_extension_runtime::mock_ext::mock_all_ext();

        let rpc = "https://rpc.ankr.com/eth_goerli";

        // Handler on Goerli
        let handler_address: H160 =
            H160::from_slice(&hex::decode("0B674CC89F54a47Be4Eb6C1A125bB8f04A529181").unwrap());
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

        let mut call = sygma_bridge
            .build_call(Step {
                exe: String::from(""),
                source_chain: String::from("Goerli"),
                dest_chain: String::from("Rhala"),
                spend_asset: hex::decode("B376b0Ee6d8202721838e76376e81eEc0e2FE864").unwrap(),
                // MuliLocation: (0, Here)
                receive_asset: hex::decode("0000").unwrap(),
                sender: Some(hex::decode("53e4C6611D3C92232bCBdd20D1073ce892D34594").unwrap()),
                recipient: hex::decode(
                    "04dba0677fc274ffaccc0fa1030a66b171d1da9226d2bb9d152654e6a746f276",
                )
                .unwrap(),
                spend_amount: Some(1_000_000_000_000_000_000 as u128),
                origin_balance: None,
                nonce: None,
            })
            .unwrap();

        // Apply index mannually
        call.input_call = Some(0);
        call.call_index = Some(0);

        // Make sure handler address hold enough spend asset and native asset (e.g. ETH).
        // Because handler is the account who spend and pay fee on behalf

        // Estiamte gas before submission
        let gas = resolve_ready(handler.estimate_gas(
            "batchCall",
            vec![call.clone()],
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
        // Goerli: https://goerli.etherscan.io/tx/0xe64402af2a358c155a15d26bd547ab6beb51790a4ffb3f0eee248b5b67e09dab
        // Rhala:

        let secret_key = std::env::vars().find(|x| x.0 == "SECRET_KEY");
        let secret_key = secret_key.unwrap().1;
        let secret_bytes = hex::decode(secret_key).unwrap();
        let signer: [u8; 32] = secret_bytes.to_array();

        let _tx_id: primitive_types::H256 = resolve_ready(handler.signed_call(
            "batchCall",
            vec![call],
            Options::with(|opt| {
                opt.gas = Some(gas);
                // 0.001 ETH
                opt.value = Some(U256::from(1_000_000_000_000_000u128))
            }),
            KeyPair::from(signer),
        ))
        .map_err(|e| {
            println!("Failed to submit step execution tx with error: {:?}", e);
            "FailedToSubmitTransaction"
        })
        .unwrap();
    }

    // cargo test --package index_executor --lib -- actions::ethereum::sygma::tests::test_batch_call_on_ethereum --exact --nocapture
    #[test]
    #[ignore]
    fn test_batch_call_on_ethereum() {
        pink_extension_runtime::mock_ext::mock_all_ext();

        let rpc = "https://mainnet.infura.io/v3/e5f4c95222934613bbde028ba5dc526b";

        // Handler on Ethereum
        let handler_address: H160 =
            H160::from_slice(&hex::decode("d693bDC5cb0cF2a31F08744A0Ec135a68C26FE1c").unwrap());
        let transport = Eth::new(PinkHttp::new(rpc.clone()));
        let handler = Contract::from_json(
            transport,
            handler_address,
            include_bytes!("../../abi/handler.json"),
        )
        .unwrap();

        let wrap_call = Call {
            params: CallParams::Evm(EvmCall {
                target: hex::decode("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")
                    .unwrap()
                    .to_array()
                    .into(),
                calldata: vec![208, 227, 13, 176],
                value: U256::from(300000000000000_u128),
                need_settle: true,
                update_offset: U256::from(0),
                update_len: U256::from(0),
                spender: hex::decode("0000000000000000000000000000000000000000")
                    .unwrap()
                    .to_array()
                    .into(),
                spend_asset: hex::decode("0000000000000000000000000000000000000000")
                    .unwrap()
                    .to_array()
                    .into(),
                spend_amount: U256::from(300000000000000_u128),
                receive_asset: hex::decode("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")
                    .unwrap()
                    .to_array()
                    .into(),
            }),
            input_call: Some(0),
            call_index: Some(0),
        };

        let swap_call = Call {
            params: CallParams::Evm(EvmCall {
                target: hex::decode("7a250d5630b4cf539739df2c5dacb4c659f2488d")
                    .unwrap()
                    .to_array()
                    .into(),
                calldata: vec![
                    56, 237, 23, 57, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 160, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 214, 147, 189, 197, 203, 12, 242, 163, 31, 8, 116,
                    74, 14, 193, 53, 166, 140, 38, 254, 28, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 101, 49, 28, 40, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 192, 42, 170, 57, 178, 35, 254, 141, 10,
                    14, 92, 79, 39, 234, 217, 8, 60, 117, 108, 194, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 108, 91, 169, 22, 66, 241, 2, 130, 181, 118, 217, 25, 34, 174, 100, 72,
                    201, 213, 47, 78,
                ],
                value: U256::from(0),
                need_settle: true,
                update_offset: U256::from(4),
                update_len: U256::from(32),
                spender: hex::decode("7a250d5630b4cf539739df2c5dacb4c659f2488d")
                    .unwrap()
                    .to_array()
                    .into(),
                spend_asset: hex::decode("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")
                    .unwrap()
                    .to_array()
                    .into(),
                spend_amount: U256::from(0),
                receive_asset: hex::decode("6c5ba91642f10282b576d91922ae6448c9d52f4e")
                    .unwrap()
                    .to_array()
                    .into(),
            }),
            input_call: Some(0),
            call_index: Some(1),
        };

        let bridge_call = Call {
            params: CallParams::Evm(EvmCall {
                target: hex::decode("4d878e8fb90178588cda4cf1dccdc9a6d2757089")
                    .unwrap()
                    .to_array()
                    .into(),
                calldata: vec![
                    115, 196, 92, 152, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 128, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 1, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 100, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 36, 0, 1, 1, 0,
                    4, 219, 160, 103, 127, 194, 116, 255, 172, 204, 15, 161, 3, 10, 102, 177, 113,
                    209, 218, 146, 38, 210, 187, 157, 21, 38, 84, 230, 167, 70, 242, 118, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 6, 90, 243, 16, 122, 64, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                ],
                value: U256::from(100000000000000_u128),
                need_settle: false,
                update_offset: U256::from(164),
                update_len: U256::from(32),
                spender: hex::decode("c832588193cd5ed2185dada4a531e0b26ec5b830")
                    .unwrap()
                    .to_array()
                    .into(),
                spend_asset: hex::decode("6c5ba91642f10282b576d91922ae6448c9d52f4e")
                    .unwrap()
                    .to_array()
                    .into(),
                spend_amount: U256::from(0),
                receive_asset: hex::decode("6c5ba91642f10282b576d91922ae6448c9d52f4e")
                    .unwrap()
                    .to_array()
                    .into(),
            }),
            input_call: Some(1),
            call_index: Some(2),
        };

        let _gas = resolve_ready(handler.estimate_gas(
            "batchCall",
            vec![wrap_call, swap_call, bridge_call],
            // Worker address
            Address::from_slice(&hex::decode("bf526928373748b00763875448ee905367d97f96").unwrap()),
            Options::default(),
        ))
        .map_err(|e| {
            println!("Failed to estimated step gas cost with error: {:?}", e);
            "FailedToEstimateGas"
        })
        .unwrap();
    }
}

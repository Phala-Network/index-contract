//! Substrate json RPC module with limited functionalites

use super::traits::Error;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use primitive_types::H256;
mod era;
mod objects;
use super::subrpc::objects::*;
mod rpc;
use pink_json as json;
use rpc::call_rpc;
use ss58_registry::Ss58AddressFormat;
mod ss58;
use ss58::Ss58Codec;

/// Gets the next nonce of the target account
///
/// Nonce represents how many transactions the account has successfully issued
/// TODO: simplify
pub fn get_next_nonce(rpc_node: &str, ss58_addr: &str) -> core::result::Result<NextNonceOk, Error> {
    // TODO: can we contruct the json object using serde_json_core?
    let data = format!(
        r#"{{"id":1,"jsonrpc":"2.0","method":"system_accountNextIndex","params":["{}"]}}"#,
        ss58_addr
    )
    .into_bytes();
    let resp_body = call_rpc(&rpc_node, data)?;

    let next_nonce: NextNonce = json::from_slice(&resp_body).or(Err(Error::InvalidBody))?;

    let next_nonce_ok = NextNonceOk {
        next_nonce: next_nonce.result,
    };

    Ok(next_nonce_ok)
}

// TODO: simplify
pub fn get_runtime_version(rpc_node: &str) -> core::result::Result<RuntimeVersionOk, Error> {
    let data = r#"{"id":1, "jsonrpc":"2.0", "method": "state_getRuntimeVersion"}"#
        .to_string()
        .into_bytes();
    let resp_body = call_rpc(&rpc_node, data)?;

    let runtime_version: RuntimeVersion =
        json::from_slice(&resp_body).or(Err(Error::InvalidBody))?;

    let runtime_version_result = runtime_version.result;
    let mut api_vec: Vec<(String, u32)> = Vec::new();
    for (api_str, api_u32) in runtime_version_result.apis {
        api_vec.push((api_str.to_string().parse().unwrap(), api_u32));
    }

    let runtime_version_ok = RuntimeVersionOk {
        spec_name: runtime_version_result.specName.to_string().parse().unwrap(),
        impl_name: runtime_version_result.implName.to_string().parse().unwrap(),
        authoring_version: runtime_version_result.authoringVersion,
        spec_version: runtime_version_result.specVersion,
        impl_version: runtime_version_result.implVersion,
        apis: api_vec,
        transaction_version: runtime_version_result.transactionVersion,
        state_version: runtime_version_result.stateVersion,
    };
    Ok(runtime_version_ok)
}

// TODO: simplify
pub fn get_genesis_hash(rpc_node: &str) -> core::result::Result<GenesisHashOk, Error> {
    let data = r#"{"id":1, "jsonrpc":"2.0", "method": "chain_getBlockHash","params":["0"]}"#
        .to_string()
        .into_bytes();
    let resp_body = call_rpc(rpc_node, data)?;
    let genesis_hash: GenesisHash = json::from_slice(&resp_body).or(Err(Error::InvalidBody))?;
    // bypass prefix 0x
    let genesis_hash_result = &genesis_hash.result[2..];
    let genesis_hash_ok = GenesisHashOk {
        genesis_hash: hex::decode(genesis_hash_result).or(Err(Error::InvalidBody))?,
    };

    Ok(genesis_hash_ok)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subrpc::ss58::get_ss58addr_version;

    /// Test data:
    ///
    /// subkey inspect 0x9eb2ee60393aeeec31709e256d448c9e40fa64233abf12318f63726e9c417b69 --scheme sr25519 --network kusama
    /// Secret Key URI `0x9eb2ee60393aeeec31709e256d448c9e40fa64233abf12318f63726e9c417b69` is account:
    ///   Network ID:        kusama
    ///   Secret seed:       0x9eb2ee60393aeeec31709e256d448c9e40fa64233abf12318f63726e9c417b69
    ///   Public key (hex):  0x8266b3183ccc58f3d145d7a4894547bd55d7739751dd15802f36ec8a0d7be314
    ///   Account ID:        0x8266b3183ccc58f3d145d7a4894547bd55d7739751dd15802f36ec8a0d7be314
    ///   Public key (SS58): FXJFWSVDcyVi3bTy8D9ESznQM4JoNBRQLEjWFgAGnGQfpbR
    ///   SS58 Address:      FXJFWSVDcyVi3bTy8D9ESznQM4JoNBRQLEjWFgAGnGQfpbR
    #[test]
    fn can_get_next_nonce() {
        pink_extension_runtime::mock_ext::mock_all_ext();
        let version = get_ss58addr_version("kusama").unwrap();
        let public_key: [u8; 32] =
            hex_literal::hex!("8266b3183ccc58f3d145d7a4894547bd55d7739751dd15802f36ec8a0d7be314")
                .into();
        let addr = public_key.to_ss58check_with_version(version.prefix());
        let next_nonce = get_next_nonce("https://kusama-rpc.polkadot.io", &addr).unwrap();
        assert!(next_nonce.next_nonce >= 0);
    }

    #[test]
    fn can_get_runtime_version() {
        pink_extension_runtime::mock_ext::mock_all_ext();
        let runtime_version = get_runtime_version("https://kusama-rpc.polkadot.io").unwrap();
        assert_eq!(runtime_version.impl_name, "parity-kusama");
    }

    #[test]
    fn can_get_genesis_hash() {
        pink_extension_runtime::mock_ext::mock_all_ext();
        let genesis_hash = get_genesis_hash("https://kusama-rpc.polkadot.io").unwrap();
        assert_eq!(
            hex::encode(genesis_hash.genesis_hash),
            "b0a8d493285c2df73290dfb7e61f870f17b41801197a149ca93654499ea3dafe"
        );
    }
}

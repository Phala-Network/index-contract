//#[allow(clippy::large_enum_variant)]
use crate::alloc::string::ToString;
use crate::chain::Chain;
use alloc::{string::String, vec::Vec};
use ink::storage::traits::StorageLayout;

#[derive(Debug, Clone, Default, PartialEq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
pub struct Asset {
    pub id: u32,
    pub symbol: String,
    pub name: String,
    pub location: String,
    pub decimals: u32,
    pub chain_id: u32,
}

#[derive(Debug, Clone, Default, PartialEq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
pub struct Dex {
    pub id: u32,
    pub name: String,
    pub chain_id: u32,
}

#[derive(Debug, Clone, Default, PartialEq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
pub struct DexIndexer {
    pub id: u32,
    pub url: String,
    pub dex_id: u32,
}

#[derive(Debug, Clone, Default, PartialEq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
pub struct DexPair {
    pub id: u32,
    pub asset0_id: u32,
    pub asset1_id: u32,
    pub dex_id: u32,
    pub pair_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
pub struct Bridge {
    pub id: u32,
    pub name: String,
    pub location: String,
}

#[derive(Debug, Clone, Default, PartialEq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
pub struct BridgePair {
    pub id: u32,
    pub asset0_id: u32,
    pub asset1_id: u32,
    pub bridge_id: u32,
}

#[derive(Debug, Clone, Default, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
pub struct Graph {
    pub chains: Vec<Chain>,
    pub assets: Vec<Asset>,
    pub dexs: Vec<Dex>,
    pub dex_pairs: Vec<DexPair>,
    pub dex_indexers: Vec<DexIndexer>,
    pub bridges: Vec<Bridge>,
    pub bridge_pairs: Vec<BridgePair>,
}

impl Graph {
    pub fn get_chain(&self, name: String) -> Option<Chain> {
        let chains = &self.chains;
        chains
            .iter()
            .position(|c| c.name == name)
            .map(|idx| chains[idx].clone())
    }
}

// some field from the first graph(the RegistryGraph) is a String that is hexified somewhere else,
// the right way to decode it is:
//  - de-hexify it to be Vec<u8>
//  - restore the string from Vec<u8>
// for example:
// - a tool hexifies a string "0x3a62a4980b952C92f4d4243c4A009336Ee0a26eB" into 33613632613439383062393532433932663464343234336334413030393333364565306132366542
// - Phat contract receives 33613632613439383062393532433932663464343234336334413030393333364565306132366542
// - Phat contract needs to decode 33613632613439383062393532433932663464343234336334413030393333364565306132366542 into 0x3a62a4980b952C92f4d4243c4A009336Ee0a26eB
// - 0x3a62a4980b952C92f4d4243c4A009336Ee0a26eB is in bytes because the hex::decode gives Vec<u8> output
// - restore string from bytes using String::from_utf8_lossy
fn hexified_to_string(hs: &str) -> core::result::Result<String, &'static str> {
    Ok(
        String::from_utf8_lossy(&hex::decode(hs).or(Err("DecodeFailed"))?)
            .to_string()
            .to_lowercase(),
    )
}

// when we restore a string from hexified string, to turn that into Vec<u8>,
// first thing is to remove the prefixing 0x, then hex::decode again
fn hexified_to_vec_u8(hs: &str) -> core::result::Result<Vec<u8>, &'static str> {
    let binding = hex::decode(hs).or(Err("DecodeFailed"))?;
    let withhead = &String::from_utf8_lossy(&binding);

    if let Some(headless) = withhead.strip_prefix("0x") {
        hex::decode(headless).or(Err("DecodeFailed"))
    } else {
        Err("wrong hex string")
    }
}

fn vec_u8_to_hexified(v: &[u8]) -> String {
    let headless = hex::encode(v);
    let withhead = String::from("0x") + &headless;
    hex::encode(withhead.as_bytes())
}

fn string_to_hexified(s: &str) -> String {
    hex::encode(s.as_bytes())
}

#[cfg(test)]
mod tests {
    use core::str::FromStr;
    use primitive_types::H160;

    use super::*;

    #[test]
    fn string_codec_should_work() {
        let input =
            "307833613632613439383062393532633932663464343234336334613030393333366565306132366562"
                .to_string();
        assert_eq!(
            "0x3a62a4980b952c92f4d4243c4a009336ee0a26eb".to_string(),
            hexified_to_string(&input).unwrap()
        );
        let v = hexified_to_vec_u8(&input).unwrap();
        assert_eq!(
            vec![
                0x3a, 0x62, 0xa4, 0x98, 0x0b, 0x95, 0x2C, 0x92, 0xf4, 0xd4, 0x24, 0x3c, 0x4A, 0x00,
                0x93, 0x36, 0xEe, 0x0a, 0x26, 0xeB
            ],
            v
        );
        let h1 = H160::from_slice(&v);
        let h2 = H160::from_str("0x3a62a4980b952c92f4d4243c4a009336ee0a26eb").unwrap();
        assert_eq!(h1, h2);

        let s = vec_u8_to_hexified(&v);

        assert_eq!(s, input);
    }
}

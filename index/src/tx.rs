use crate::prelude::Error;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use pink_extension::http_post;
use scale::{Decode, Encode};
use serde::Deserialize;

#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub block_number: u64,
    pub id: String,
    pub nonce: u64,
    pub result: bool,
    pub timestamp: String,
}

#[derive(Debug, Deserialize)]
struct Response {
    data: Data,
}

#[derive(Debug, Deserialize)]
struct Data {
    transactions: Vec<Transaction>,
}

pub fn get_tx_by_nonce(
    indexer: &str,
    nonce: u32,
) -> core::result::Result<Option<Transaction>, Error> {
    let query = format!(
        r#"{{ 
            "query": "query Query {{ transactions(where: {{nonce_eq: {nonce}}}) {{ blockNumber id nonce result timestamp }} }}",
            "variables": null,
            "operationName": "Query"
        }}"#
    );
    let content_length = format!("{}", query.len());
    let headers: Vec<(String, String)> = vec![
        ("Content-Type".into(), "application/json".into()),
        ("Content-Length".into(), content_length),
    ];
    let response = http_post!(indexer, query, headers);

    if response.status_code != 200 {
        return Err(Error::CallIndexerFailed);
    }

    dbg!(String::from_utf8_lossy(&response.body));
    let body: Response = pink_json::from_slice(&response.body).unwrap(); //.or(Err(Error::InvalidBody))?;
    Ok(Some(body.data.transactions[0].clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn indexer_works() {
        pink_extension_runtime::mock_ext::mock_all_ext();
        let tx = get_tx_by_nonce("https://squid.subsquid.io/squid-acala/v/v1/graphql", 20).unwrap();
        assert_eq!(tx.unwrap().nonce, 20);
    }

    #[test]
    fn deserializable() {
        let json_string = "{\"data\":{\"transactions\":[{\"blockNumber\":3114089,\"id\":\"0003114089-000002-2492e\",\"nonce\":20,\"result\":true,\"timestamp\":\"2023-03-10T06:27:18.301000Z\"}]}}\n";
        let response: Response = pink_json::from_str(json_string).unwrap();

        println!("{:#?}", response);
    }
}

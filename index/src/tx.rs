use crate::prelude::Error;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use pink_extension::http_post;
use scale::Decode;
use serde::Deserialize;

#[derive(Clone, Debug, PartialEq)]
pub struct Transaction {
    pub block_number: u64,
    pub id: String,
    pub nonce: u64,
    pub result: bool,
    // unix timestamp
    pub timestamp: u64,
    pub account: Vec<u8>,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
#[serde(rename_all = "camelCase")]
/// online transaction structure
struct Tx {
    pub block_number: u64,
    pub id: String,
    pub nonce: u64,
    pub result: bool,
    pub timestamp: String,
    pub account: Account,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
struct Account {
    id: String,
}

#[derive(Debug, Deserialize)]
struct Response {
    data: Data,
}

#[derive(Debug, Deserialize)]
struct Data {
    transactions: Vec<Tx>,
}

pub fn get_tx(
    indexer: &str,
    account: &[u8],
    nonce: u64,
) -> core::result::Result<Option<Transaction>, Error> {
    let account = format!("0x{}", hex::encode(account));
    let query = format!(
        r#"{{ 
            "query": "query Query {{ transactions(where: {{nonce_eq: {nonce}, account: {{id_eq: \"{account}\"}} }}) {{ blockNumber id nonce result timestamp account {{ id }} }} }}",
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

    let body: Response = pink_json::from_slice(&response.body).or(Err(Error::InvalidBody))?;
    let transactions = &body.data.transactions;

    if transactions.len() != 1 {
        return Err(Error::TransactionNotFound);
    }

    let tx = &body.data.transactions[0];

    Ok(Some(Transaction {
        block_number: tx.block_number,
        id: tx.id.clone(),
        nonce: tx.nonce,
        result: tx.result,
        // subsquid actually displays BigInt as string
        timestamp: tx.timestamp.parse::<u64>().or(Err(Error::InvalidBody))?,
        account: hex::decode(&tx.account.id[2..]).or(Err(Error::InvalidAddress))?,
    }))
}

pub fn is_tx_ok(indexer: &str, account: &[u8], nonce: u64) -> Result<bool, Error> {
    let tx = get_tx(indexer, account, nonce)?;
    if let Some(tx) = tx {
        return Ok(tx.result);
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn indexer_works() {
        pink_extension_runtime::mock_ext::mock_all_ext();
        let account =
            hex_literal::hex!("cee6b60451fe18916873a0775b8ab8535843b90b1d92ccc1b75925c375790623");
        let tx = get_tx(
            "https://squid.subsquid.io/squid-acala/v/v1/graphql",
            &account,
            16,
        )
        .unwrap();
        dbg!(&tx);
        assert_eq!(tx.unwrap().nonce, 16);
    }

    #[test]
    fn decoding_works() {
        let response = "{\"data\":{\"transactions\":[{\"blockNumber\":2709567,\"id\":\"0002709567-000002-37a14\",\"nonce\":16,\"result\":true,\"timestamp\":\"1673514894473\",\"account\":{\"id\":\"0xcee6b60451fe18916873a0775b8ab8535843b90b1d92ccc1b75925c375790623\"}}]}}\n";
        let response: Response = pink_json::from_str(response).unwrap();
        dbg!(response);
    }
}

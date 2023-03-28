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
    pub timestamp: String,
    pub account: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DepositEvent {
    pub block_number: u64,
    pub index_in_block: u64,
    pub id: String,
    pub result: bool,
    pub amount: u128,
    pub name: String,
    // unix timestamp
    pub timestamp: String,
    pub account: Vec<u8>,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
struct Event {
    pub id: String,
    pub name: String,
    pub amount: String,
    pub account: Account,
    pub result: bool,
    pub block_number: u64,
    pub index_in_block: u64,
    pub timestamp: String,
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

#[derive(Debug, Deserialize)]
struct DepositEventResponse {
    data: DepositEventData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DepositEventData {
    deposit_events: Vec<Event>,
}

fn indexer_rpc(indexer: &str, query: &str) -> core::result::Result<Vec<u8>, Error> {
    let content_length = format!("{}", query.len());
    let headers: Vec<(String, String)> = vec![
        ("Content-Type".into(), "application/json".into()),
        ("Content-Length".into(), content_length),
    ];
    let response = http_post!(indexer, query, headers);

    if response.status_code != 200 {
        return Err(Error::CallIndexerFailed);
    }

    Ok(response.body)
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
    let body = indexer_rpc(indexer, &query)?;
    let response: Response = pink_json::from_slice(&body).or(Err(Error::InvalidBody))?;
    let transactions = &response.data.transactions;

    if transactions.len() != 1 {
        return Err(Error::TransactionNotFound);
    }

    let tx = &response.data.transactions[0];

    Ok(Some(Transaction {
        block_number: tx.block_number,
        id: tx.id.clone(),
        nonce: tx.nonce,
        result: tx.result,
        timestamp: tx.timestamp.clone(),
        account: hex::decode(&tx.account.id[2..]).or(Err(Error::InvalidAddress))?,
    }))
}

pub fn get_deposit_event(
    indexer: &str,
    account: &[u8],
    timestamp: &str,
) -> core::result::Result<Option<DepositEvent>, Error> {
    let account = format!("0x{}", hex::encode(account));
    let query = format!(
        r#"{{ 
            "query": "query Query {{ depositEvents(where: {{account: {{id_eq: \"{account}\"}}, timestamp_gt: \"{timestamp}\" }}) {{ id name amount account {{ id }} result blockNumber indexInBlock timestamp }} }}",
            "variables": null,
            "operationName": "Query"
        }}"#
    );
    let body = indexer_rpc(indexer, &query)?;
    let response: DepositEventResponse =
        pink_json::from_slice(&body).or(Err(Error::InvalidBody))?;
    let events = &response.data.deposit_events;

    if events.len() != 1 {
        return Err(Error::DepositEventNotFound);
    }

    let ev = &response.data.deposit_events[0];

    Ok(Some(DepositEvent {
        id: ev.id.clone(),
        name: ev.name.clone(),
        amount: ev.amount.parse::<u128>().or(Err(Error::InvalidAmount))?,
        account: hex::decode(&ev.account.id[2..]).or(Err(Error::InvalidAddress))?,
        result: ev.result,
        index_in_block: ev.index_in_block,
        block_number: ev.block_number,
        timestamp: ev.timestamp.clone(),
    }))
}

pub fn get_lastest_timestamp(indexer: &str, account: &[u8]) -> Result<String, Error> {
    let account = format!("0x{}", hex::encode(account));
    let query = format!(
        r#"{{ 
            "query": "query Query {{ depositEvents(where: {{account: {{id_eq: \"{account}\"}} }}, orderBy: timestamp_DESC, limit: 1) {{ id name amount account {{ id }} result blockNumber indexInBlock timestamp }} }}",
            "variables": null,
            "operationName": "Query"
        }}"#
    );
    let body = indexer_rpc(indexer, &query)?;
    let response: DepositEventResponse =
        pink_json::from_slice(&body).or(Err(Error::InvalidBody))?;
    let events = response.data.deposit_events;

    if events.len() != 1 {
        return Err(Error::DepositEventNotFound);
    }
    let timestamp = &events[0].timestamp;

    Ok(timestamp.into())
}

pub fn is_tx_ok(indexer: &str, account: &[u8], nonce: u64) -> Result<bool, Error> {
    let tx = get_tx(indexer, account, nonce)?;
    if let Some(tx) = tx {
        return Ok(tx.result);
    }

    Ok(false)
}

pub fn is_bridge_tx_ok(
    account: &[u8],
    src_indexer: &str,
    src_nonce: u64,
    dest_indexer: &str,
    dest_amount: u128,
    dest_timestamp: &str,
) -> Result<bool, Error> {
    // check if source tx is ok
    if !is_tx_ok(src_indexer, account, src_nonce)? {
        return Ok(false);
    }
    // check if on dest chain the recipient has a corresponding event
    let event = get_deposit_event(dest_indexer, account, dest_timestamp)?;
    if let Some(event) = event {
        if event.amount < dest_amount && event.amount == 0 {
            return Ok(event.result);
        }
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

        let response = "{\"data\":{\"depositEvents\":[{\"id\":\"0003204876-000006-7c4d3\",\"name\":\"Tokens.Deposited\",\"amount\":\"577018997\",\"account\":{\"id\":\"0xcee6b60451fe18916873a0775b8ab8535843b90b1d92ccc1b75925c375790623\"},\"result\":true,\"blockNumber\":3204876,\"indexInBlock\":6,\"timestamp\":\"1679593572593\"}]}}\n";
        let response: DepositEventResponse = pink_json::from_str(response).unwrap();
        dbg!(response);
    }

    #[test]
    fn get_event() {
        pink_extension_runtime::mock_ext::mock_all_ext();

        let account =
            hex_literal::hex!("cee6b60451fe18916873a0775b8ab8535843b90b1d92ccc1b75925c375790623");
        let event = get_deposit_event(
            "https://squid.subsquid.io/squid-acala/v/v1/graphql",
            &account,
            "1679593044391",
        )
        .unwrap()
        .unwrap();
        dbg!(event);
    }

    #[test]
    fn get_timestamp() {
        pink_extension_runtime::mock_ext::mock_all_ext();

        let account =
            hex_literal::hex!("cee6b60451fe18916873a0775b8ab8535843b90b1d92ccc1b75925c375790623");
        let timestamp = get_lastest_timestamp(
            "https://squid.subsquid.io/squid-acala/v/v1/graphql",
            &account,
        )
        .unwrap();
        dbg!(timestamp);
    }
}

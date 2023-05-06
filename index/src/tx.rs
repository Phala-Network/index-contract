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

#[derive(Debug)]
pub struct BlockInfo {
    pub block_number: u64,
    pub index_in_block: u64,
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
    let account = format!("0x{}", hex::encode(account)).to_lowercase();
    pink_extension::debug!("get_tx: begin: account: {}, nonce: {}", account, nonce);
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

    pink_extension::debug!("get_tx: got transaction: {:?}", transactions);

    if transactions.len() != 1 {
        return Ok(None);
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

pub fn get_deposit_events_by_block_info(
    indexer: &str,
    account: &[u8],
    block_number: u64,
    index_in_block: u64,
) -> core::result::Result<Vec<DepositEvent>, Error> {
    let account = format!("0x{}", hex::encode(account)).to_lowercase();
    let query = format!(
        r#"{{ 
            "query": "query Query {{ depositEvents(where: {{ AND: [ {{ account: {{ id_eq: \"{account}\" }} }} {{ OR: [ {{ AND: [ {{ blockNumber_eq: {block_number} }}, {{ indexInBlock_gt: {index_in_block} }} ] }} {{ blockNumber_gt: {block_number} }} ]}} ] }} orderBy: [blockNumber_ASC, indexInBlock_ASC]) {{ id name amount account {{ id }} result blockNumber indexInBlock timestamp }} }}",
            "variables": null,
            "operationName": "Query"
        }}"#
    );

    pink_extension::debug!("is_bridge_dest_tx_ok: query: {}", query);
    let body = indexer_rpc(indexer, &query)?;
    let response: DepositEventResponse =
        pink_json::from_slice(&body).or(Err(Error::InvalidBody))?;

    let mut devents = vec![];
    for ev in response.data.deposit_events {
        let dev = DepositEvent {
            block_number: ev.block_number,
            index_in_block: ev.index_in_block,
            id: ev.id,
            result: ev.result,
            amount: ev.amount.parse::<u128>().or(Err(Error::InvalidAmount))?,
            name: ev.name,
            timestamp: ev.timestamp,
            account: hex::decode(&ev.account.id[2..]).or(Err(Error::InvalidAddress))?,
        };
        devents.push(dev);
    }
    Ok(devents)
}

pub fn get_lastest_timestamp(indexer: &str, account: &[u8]) -> Result<String, Error> {
    let account = format!("0x{}", hex::encode(account)).to_lowercase();
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

pub fn get_latest_event_block_info(indexer: &str, account: &[u8]) -> Result<BlockInfo, Error> {
    let account = format!("0x{}", hex::encode(account)).to_lowercase();
    let query = format!(
        r#"{{ 
            "query": "query Query {{ depositEvents(where: {{account: {{id_eq: \"{account}\"}} }}, orderBy: [blockNumber_DESC, indexInBlock_DESC], limit: 1) {{ id name account {{ id }} amount result indexInBlock blockNumber timestamp }} }}",
            "variables": null,
            "operationName": "Query"
        }}"#
    );
    pink_extension::debug!("get_latest_event_block_info: query: {}", query);
    let body = indexer_rpc(indexer, &query)?;
    let response: DepositEventResponse =
        pink_json::from_slice(&body).or(Err(Error::InvalidBody))?;
    let events = response.data.deposit_events;

    if events.is_empty() {
        return Ok(BlockInfo {
            block_number: 0,
            index_in_block: 0,
        });
    }

    if events.len() != 1 {
        return Err(Error::DepositEventNotFound);
    }

    let block_number = events[0].block_number;
    let index_in_block = events[0].index_in_block;

    Ok(BlockInfo {
        block_number,
        index_in_block,
    })
}

// the nonce given to this API is an expected value
pub fn is_tx_ok(indexer: &str, account: &[u8], nonce: u64) -> Result<bool, Error> {
    // nonce from storage is one larger than the last tx's nonce
    pink_extension::debug!("is_tx_ok: begin");
    let tx = get_tx(indexer, account, nonce)?;
    pink_extension::debug!("is_tx_ok: got tx: {:?}", tx);
    if let Some(tx) = tx {
        return Ok(tx.result);
    }

    Ok(false)
}

#[allow(clippy::too_many_arguments)]
pub fn is_bridge_tx_ok(
    src_account: &[u8],
    dest_account: &[u8],
    src_indexer: &str,
    src_nonce: u64,
    dest_indexer: &str,
    receive_min: u128,
    receive_max: u128,
    block_number: u64,
    index_in_block: u64,
) -> Result<(bool, (u64, u64)), Error> {
    pink_extension::debug!("is_bridge_tx_ok: begin");
    // check if source tx is ok
    if !is_tx_ok(src_indexer, src_account, src_nonce)? {
        return Ok((false, (block_number, index_in_block)));
    }
    pink_extension::debug!("is_bridge_tx_ok: source chain tx is ok! now check the dest chain tx");
    // check if dest tx is ok
    is_bridge_dest_tx_ok(
        dest_account,
        dest_indexer,
        receive_min,
        receive_max,
        block_number,
        index_in_block,
    )
}

fn is_bridge_dest_tx_ok(
    account: &[u8],
    dest_indexer: &str,
    receive_min: u128,
    receive_max: u128,
    block_number: u64,
    index_in_block: u64,
) -> Result<(bool, (u64, u64)), Error> {
    pink_extension::debug!("is_bridge_dest_tx_ok: begin: indexer: {}", dest_indexer);
    // check if on dest chain the recipient has a corresponding event
    let events =
        get_deposit_events_by_block_info(dest_indexer, account, block_number, index_in_block)?;

    pink_extension::debug!("is_bridge_dest_tx_ok: events: {:?}", events);

    for event in events {
        if receive_min < event.amount && receive_max > event.amount {
            // event found! the block info should be saved to the next step
            return Ok((true, (event.block_number, event.index_in_block)));
        }
    }

    pink_extension::debug!("is_bridge_dest_tx_ok: exit");
    Ok((false, (block_number, index_in_block)))
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
            "https://squid.subsquid.io/graph-acala/v/v1/graphql",
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
    fn get_timestamp() {
        pink_extension_runtime::mock_ext::mock_all_ext();

        let account =
            hex_literal::hex!("cee6b60451fe18916873a0775b8ab8535843b90b1d92ccc1b75925c375790623");
        let timestamp = get_lastest_timestamp(
            "https://squid.subsquid.io/graph-acala/v/v1/graphql",
            &account,
        )
        .unwrap();
        dbg!(timestamp);
    }

    #[test]
    fn get_block_info() {
        pink_extension_runtime::mock_ext::mock_all_ext();

        let account =
            hex_literal::hex!("8119850bdfdf9792ec2f26a940b58fe43bfb083298ac8035d33a30ea4e5695fe");
        let block_info = get_latest_event_block_info(
            "https://squid.subsquid.io/graph-acala/v/v1/graphql",
            &account,
        )
        .unwrap();
        dbg!(block_info);
    }

    #[test]
    fn get_events() {
        pink_extension_runtime::mock_ext::mock_all_ext();

        let account =
            hex_literal::hex!("8119850bdfdf9792ec2f26a940b58fe43bfb083298ac8035d33a30ea4e5695fe");
        let events = get_deposit_events_by_block_info(
            "https://squid.subsquid.io/graph-acala/v/v1/graphql",
            &account,
            0,
            0,
        )
        .unwrap();
        dbg!(events);
    }

    #[test]
    fn checker_works() {
        // https://acala.subscan.io/extrinsic/2705255-1?event=2705255-10
        // receive: 528352559
        pink_extension_runtime::mock_ext::mock_all_ext();

        let account =
            hex_literal::hex!("cee6b60451fe18916873a0775b8ab8535843b90b1d92ccc1b75925c375790623");

        // the aim is the catch the first event
        let res = is_bridge_dest_tx_ok(
            &account,
            "https://squid.subsquid.io/graph-acala/v/v1/graphql",
            500_352_559,
            600_352_559,
            3204833,
            7,
        )
        .unwrap();
        dbg!(res);
        assert!(res.0);

        let a32 =
            hex_literal::hex!("12735d5f5ddf9a3153d744fdd98ab77f7f181aa30101b09cc694cbf18470956c");
        let a20 = hex_literal::hex!("0dc509699299352c57080cf27128765a5cab8800");
        dbg!(hex::encode(&a20));
        dbg!(hex::encode(&a32));

        // the aim is the catch the first event
        let res = is_bridge_tx_ok(
            &a20,
            &a32,
            "https://squid.subsquid.io/graph-moonbeam/v/v1/graphql",
            10,
            "https://squid.subsquid.io/graph-acala/v/v1/graphql",
            0,
            10000000,
            0,
            0,
        )
        .unwrap();
        dbg!(res);
    }
}

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use pink_extension::http_post;

use crate::index_executor::Error;

pub fn get_tx_by_nonce(indexer: &str, nonce: u32) -> core::result::Result<Vec<u8>, Error> {
    let query = format!(
        r#"{{ 
            "query": "query Query {{ transactions(where: {{nonce_eq: {nonce}}}) {{ blockNumber id nonce result timestamp }} }}",
            "variables": null,
            "operationName": "Query"
        }}"#
    );

    println!("{}", query.clone());
    let content_length = format!("{}", query.len());
    let headers: Vec<(String, String)> = vec![
        ("Content-Type".into(), "application/json".into()),
        ("Content-Length".into(), content_length),
    ];
    let response = http_post!(indexer, query, headers);

    dbg!(String::from_utf8_lossy(&response.body));

    if response.status_code != 200 {
        return Err(Error::CallIndexerFailed);
    }

    Ok(response.body)
}

#[cfg(test)]
mod tests {
    use super::get_tx_by_nonce;
    #[test]
    fn indexer_works() {
        pink_extension_runtime::mock_ext::mock_all_ext();

        let tx = get_tx_by_nonce("https://squid.subsquid.io/squid-acala/v/v1/graphql", 20).unwrap();
    }
}

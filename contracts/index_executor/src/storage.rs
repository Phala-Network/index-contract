use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use pink_extension::http_req;
use scale::Decode;
use serde::Deserialize;

#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
#[serde(rename_all = "camelCase")]
struct StringItem {
    pub string_value: String,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
#[serde(rename_all = "camelCase")]
struct EmptyData {
    pub read_time: String,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
#[serde(rename_all = "camelCase")]
struct DataFields {
    pub data: StringItem,
    pub id: StringItem,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
#[serde(rename_all = "camelCase")]
struct Document {
    pub name: String,
    pub fields: DataFields,
    pub create_time: String,
    pub update_time: String,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
#[serde(rename_all = "camelCase")]
struct ResponseData {
    pub document: Document,
    pub read_time: String,
}

/// The client that interacts with cloud storage services.
/// - `url`: The base URL of the service REST endpoint
/// - `key`: The credentials associated with the service, generally the access token
///
/// By concacting the `url` with the specific API path, we can get a full REST API endpoint.
/// The result of every request will returned immediately along with the HTTP response.
///
/// **Note** The `key` should be kept secret when passing from other modules
pub struct StorageClient {
    url: String,
    key: String,
}

impl StorageClient {
    pub fn new(url: String, key: String) -> Self {
        StorageClient { url, key }
    }

    /// Send a request to the storage service according to the REST API specification
    fn send_request(
        &self,
        method: &str,
        api: &str,
        request: &str,
    ) -> Result<Vec<u8>, &'static str> {
        let content_length = format!("{}", request.len());
        let access_key = format!("Bearer {}", self.key);
        let headers: Vec<(String, String)> = vec![
            ("Content-Type".into(), "application/json".into()),
            ("Authorization".into(), access_key),
            ("Content-Length".into(), content_length),
        ];

        let response: pink_extension::chain_extension::HttpResponse = http_req!(
            method,
            format!("{}{}", self.url.clone(), api),
            request.as_bytes().to_vec(),
            headers
        );
        if response.status_code != 200 {
            return Err("CallServiceFailed");
        }

        Ok(response.body)
    }

    /// Return (data, document_id) if success
    pub fn read_storage<T: Decode>(&self, key: &[u8]) -> Result<Option<(T, String)>, &'static str> {
        let key = key
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        pink_extension::debug!("read_storage: trying to read storage item, key: {}", key);

        let cmd = format!(
            r#"{{
                "structuredQuery": {{
                    "from": [{{
                      "collectionId": "index-storage"
                    }}],
                    "where": {{
                      "fieldFilter": {{
                        "field": {{
                          "fieldPath": "id"
                        }},
                        "op": "EQUAL",
                        "value": {{
                          "stringValue": "{key}"
                        }}
                      }}
                    }},
                    "limit": 1
                  }}
            }}"#
        );

        let response_body: Vec<u8> = self.send_request("POST", "documents:runQuery", &cmd)?;
        if let Ok(response) = pink_json::from_slice::<Vec<ResponseData>>(&response_body) {
            Ok(if !response.is_empty() {
                let data_str = response[0].document.fields.data.string_value.clone();
                let raw_data = hex::decode(data_str).map_err(|_| "InvalidDataStr")?;
                let data: T =
                    T::decode(&mut raw_data.as_slice()).map_err(|_| "DecodeDataFailed")?;
                let document_id = response[0]
                    .document
                    .name
                    .split('/')
                    .last()
                    .ok_or("ParseDocumentFailed")?
                    .to_string();
                Some((data, document_id))
            } else {
                None
            })
        } else {
            // Trying decode from EmptyData, this is highly related to the response format of the storage service
            if pink_json::from_slice::<Vec<EmptyData>>(&response_body).is_ok() {
                pink_extension::debug!("read_storage: no storage item found: {}", key);
                Ok(None)
            } else {
                // Here we can make sure we got unexpected data
                Err("DecodedDataFailed")
            }
        }
    }

    /// Create a new storage item
    pub fn alloc_storage(&self, key: &[u8], data: &[u8]) -> Result<(), &'static str> {
        let key: String = key
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        pink_extension::debug!(
            "alloc_storage: trying to create storage item, key: {:?}",
            key
        );
        let data_str = data
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let cmd = format!(
            r#"{{
                "fields": {{
                    "id": {{
                      "stringValue": "{key}"
                    }},
                    "data": {{
                      "stringValue": "{data_str}"
                    }}
                }}
            }}"#
        );
        let api = "documents/index-storage".to_string();
        let _ = self.send_request("POST", &api[..], &cmd)?;

        Ok(())
    }

    /// Update storage data
    pub fn update_storage(
        &self,
        key: &[u8],
        data: &[u8],
        document: String,
    ) -> Result<(), &'static str> {
        let key: String = key
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        pink_extension::debug!(
            "update_storage: trying to update storage item, key: {}",
            &key
        );
        let data_str = data
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();

        let cmd = format!(
            r#"{{
                "fields": {{
                    "id": {{
                      "stringValue": "{key}"
                    }},
                    "data": {{
                      "stringValue": "{data_str}"
                    }}
                }}
            }}"#
        );
        let api = format!("documents/index-storage/{document}");
        let _ = self.send_request("PATCH", &api[..], &cmd)?;

        Ok(())
    }

    /// Remove a document from remote storage
    pub fn remove_storage(&self, _key: &[u8], document: String) -> Result<(), &'static str> {
        let api = format!("documents/index-storage/{document}");
        let _ = self.send_request("DELETE", &api[..], "")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::{Task, TaskStatus};
    use dotenv::dotenv;
    use scale::Encode;

    // cargo test --package index_executor --lib -- storage::tests::should_work --exact --nocapture
    #[test]
    #[ignore]
    fn should_work() {
        dotenv().ok();
        pink_extension_runtime::mock_ext::mock_all_ext();
        let base_url = "https://firestore.googleapis.com/v1/projects/plexiform-leaf-391708/databases/(default)/".to_string();
        let access_token = "put your access token".to_string();

        let client = StorageClient::new(base_url, access_token);

        let mut task = Task {
            id: [1; 32],
            worker: [0; 32],
            status: TaskStatus::Actived,
            source: "Ethereum".to_string(),
            claim_nonce: None,
            steps: vec![],
            execute_index: 0,
            sender: vec![],
            recipient: vec![],
            retry_counter: 0,
        };

        assert_eq!(client.read_storage::<Task>(&task.id).unwrap(), None);
        // Save task to remote storage
        assert_eq!(
            client.alloc_storage(b"pending_tasks", &vec![task.id].encode()),
            Ok(())
        );
        assert_eq!(client.alloc_storage(&task.id, &task.encode()), Ok(()));
        // Query storage for tasks
        let (storage_task, document_id) = client.read_storage::<Task>(&task.id).unwrap().unwrap();
        assert_eq!(storage_task.encode(), task.encode());
        // Modify task status
        task.status = TaskStatus::Completed;
        // Update task data on remote storage
        assert_eq!(
            client.update_storage(&task.id, &task.encode(), document_id),
            Ok(())
        );
        // Read again
        let (updated_storage_task, document_id) =
            client.read_storage::<Task>(&task.id).unwrap().unwrap();
        assert_eq!(updated_storage_task.status, TaskStatus::Completed);
        // Delete task data from remote storage
        assert_eq!(
            client.remove_storage(&updated_storage_task.id, document_id),
            Ok(())
        );
        // Veify task existence
        assert_eq!(client.read_storage::<Task>(&task.id).unwrap(), None);
    }
}

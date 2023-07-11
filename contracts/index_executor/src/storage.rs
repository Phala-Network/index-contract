use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use pink_extension::http_req;
use scale::{Decode, Encode};
use serde::Deserialize;

use crate::task::{Task, TaskId};

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

    /// Return (encoded_data, document_id) if success
    fn read_storage(&self, key: &[u8]) -> Result<Option<(Vec<u8>, String)>, &'static str> {
        let key = key
            .iter()
            .map(|byte| format!("{:02x}", byte))
            .collect::<String>();
        pink_extension::debug!("read_storage: id: {}", key);

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
            Ok(if response.len() > 0 {
                let data_str = response[0].document.fields.data.string_value.clone();
                let data = hex::decode(&data_str).map_err(|_| "DecodedDataFailed")?;
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
            if let Ok(_) = pink_json::from_slice::<Vec<EmptyData>>(&response_body) {
                pink_extension::debug!("read_storage: no storage item found: {}", key);
                Ok(None)
            } else {
                // Here we can make sure we got unexpected data
                Err("DecodedDataFailed")
            }
        }
    }

    /// Update storage data if necessary, will create a new record if storage item does not exist
    fn write_storage(&self, key: &[u8], data: &Vec<u8>) -> Result<(), &'static str> {
        let storage_data: Option<(Vec<u8>, String)> = self.read_storage(key)?;
        let key = key
            .iter()
            .map(|byte| format!("{:02x}", byte))
            .collect::<String>();
        let data_str = data
            .iter()
            .map(|byte| format!("{:02x}", byte))
            .collect::<String>();
        let api: String;

        pink_extension::debug!("write_storage: id: {}", &key);

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
        if storage_data.is_some() {
            pink_extension::debug!("write_storage: storage item already exist in with document id: {}, update it if necessary", storage_data.clone().unwrap().1);
            if &storage_data.clone().unwrap().0 == data {
                pink_extension::debug!("write_storage: same storage data, ignore");
                return Ok(());
            }
            api = format!("documents/index-storage/{}", storage_data.unwrap().1);
            let _ = self.send_request("PATCH", &api[..], &cmd)?;
        } else {
            pink_extension::debug!(
                "write_storage: storage item doesn't exist in storage, trying to create one"
            );
            api = "documents/index-storage".to_string();
            let _ = self.send_request("POST", &api[..], &cmd)?;
        }

        Ok(())
    }

    /// Remove a document from remote storage
    fn remove_storage_item(&self, key: &[u8]) -> Result<(), &'static str> {
        let storage_data: Option<(Vec<u8>, String)> = self.read_storage(key)?;
        if storage_data.is_some() {
            let api = format!("documents/index-storage/{}", storage_data.unwrap().1);
            let _ = self.send_request("DELETE", &api[..], "")?;
        }
        Ok(())
    }

    /// Put or update a storage item to the remote storage
    pub fn put(&self, key: &[u8], data: &Vec<u8>) -> Result<(), &'static str> {
        self.write_storage(key, data)
    }

    /// Get a storage item from remote storage
    pub fn get(&self, key: &[u8]) -> Result<Vec<u8>, &'static str> {
        if let Some((storage_data, _)) = self.read_storage(key)? {
            return Ok(storage_data);
        }
        Err("StorageItemNotFound")
    }

    /// Delete a storage item from remote storage
    pub fn delete(&self, key: &[u8]) -> Result<(), &'static str> {
        self.remove_storage_item(key)
    }

    /// Upload task data to remote storage
    pub fn upload_task(&self, task: &Task) -> Result<(), &'static str> {
        self.write_storage(&task.id, &task.encode())
    }

    /// Lookup task data from remote storage, return None if not found
    pub fn lookup_task(&self, id: &TaskId) -> Option<Task> {
        if let Ok(Some((storage_data, _))) = self.read_storage(id) {
            return match Decode::decode(&mut storage_data.as_slice()) {
                Ok(task) => Some(task),
                _ => None,
            };
        }
        None
    }

    /// Lookup pending task from remote storage, return a list of pending task id
    pub fn lookup_pending_tasks(&self) -> Vec<TaskId> {
        if let Ok(Some((storage_data, _))) = self.read_storage(b"pending-tasks") {
            return match Decode::decode(&mut storage_data.as_slice()) {
                Ok(task_ids) => task_ids,
                _ => vec![],
            };
        }
        vec![]
    }

    /// Return None if worker account has not been setup
    pub fn lookup_free_accounts(&self) -> Option<Vec<[u8; 32]>> {
        if let Ok(Some((storage_data, _))) = self.read_storage(b"worker-accounts") {
            return match Decode::decode(&mut storage_data.as_slice()) {
                Ok(worker_accounts) => worker_accounts,
                _ => None,
            };
        }
        None
    }

    /// Setup worker account to remote storage
    pub fn set_worker_accounts(&self, accounts: Vec<[u8; 32]>) -> Result<(), &'static str> {
        self.write_storage(b"worker-accounts", &accounts.encode())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::TaskStatus;
    use dotenv::dotenv;

    // cargo test --package index_executor --lib -- storage::tests::should_work --exact --nocapture
    #[test]
    #[ignore]
    fn should_work() {
        dotenv().ok();
        pink_extension_runtime::mock_ext::mock_all_ext();
        let base_url = "https://firestore.googleapis.com/v1/projects/plexiform-leaf-391708/databases/(default)/".to_string();
        let access_token = "put access token here".to_string();

        let client = StorageClient::new(base_url, access_token);

        let mut task = Task {
            id: [1; 32],
            worker: [0; 32],
            status: TaskStatus::Actived,
            source: "Ethereum".to_string(),
            steps: vec![],
            execute_index: 0,
            sender: vec![],
            recipient: vec![],
        };

        assert_eq!(client.get(&task.id), Err("StorageItemNotFound"));
        assert_eq!(client.lookup_task(&task.id), None);
        // Save task to remote storage
        assert_eq!(
            client.put(b"pending-tasks", &vec![task.id].encode()),
            Ok(())
        );
        assert_eq!(client.put(&task.id, &task.encode()), Ok(()));
        assert_eq!(client.get(&task.id), Ok(task.encode()));
        // Modify task status
        task.status = TaskStatus::Completed;
        // Update task data on remote storage
        assert_eq!(client.upload_task(&task), Ok(()));
        let remote_task = client.lookup_task(&task.id).unwrap();
        assert_eq!(remote_task.status, TaskStatus::Completed);
        assert_eq!(client.lookup_pending_tasks(), vec![task.id]);
        // Delete task data from remote storage
        assert_eq!(client.delete(&task.id), Ok(()));
        assert_eq!(client.lookup_task(&task.id), None);
    }
}

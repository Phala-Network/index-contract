use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use chrono::DateTime;
use pink_extension::{chain_extension::HttpResponse, http_req, ResultExt};
use regex::Regex;
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
struct IntegerItem {
    pub integer_value: String,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
#[serde(rename_all = "camelCase")]
struct DataFields {
    pub data: StringItem,
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
struct AuditIndexFields {
    pub value: IntegerItem,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
#[serde(rename_all = "camelCase")]
struct AuditIndex {
    pub name: String,
    pub fields: AuditIndexFields,
    pub create_time: String,
    pub update_time: String,
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
    database: String,
}

impl StorageClient {
    pub fn new(url: String, key: String) -> Self {
        let re = Regex::new(r"projects/[^/]+/databases/[^/]+").unwrap();
        let database = re.find(&url).unwrap().as_str().to_string();
        StorageClient { url, key, database }
    }

    /// Send a request to the storage service according to the REST API specification
    fn send_request(
        &self,
        method: &str,
        api: &str,
        request: Option<&str>,
    ) -> Result<HttpResponse, &'static str> {
        let access_key = format!("Bearer {}", self.key);
        let headers: Vec<(String, String)> = vec![
            ("Content-Type".into(), "application/json".into()),
            ("Authorization".into(), access_key),
        ];

        let response: pink_extension::chain_extension::HttpResponse = http_req!(
            method,
            format!("{}{}", self.url.clone(), api),
            match request {
                Some(request) => request.as_bytes().to_vec(),
                None => vec![],
            },
            headers
        );
        if response.status_code != 200 && response.status_code != 404 {
            return Err("CallServiceFailed");
        }

        Ok(response)
    }

    fn get_timestamp(&self, response: &HttpResponse) -> Result<String, &'static str> {
        if let Some((_, value)) = response.headers.iter().find(|(key, _)| key == "date") {
            let timestamp = DateTime::parse_from_rfc2822(value)
                .map_err(|_| "GetTimestampFailed")?
                .to_rfc3339();
            Ok(timestamp)
        } else {
            Err("GetTimestampFailed")
        }
    }

    fn fetch_audit_index(&self) -> Result<(u64, String), &'static str> {
        let api = "documents/index-storage/audit-index";
        let response: HttpResponse = self.send_request("GET", api, None)?;
        let timestamp: String = self.get_timestamp(&response)?;

        if response.status_code == 404 {
            Ok((0, timestamp))
        } else if let Ok(response) = pink_json::from_slice::<AuditIndex>(&response.body) {
            let current_index: u64 = response.fields.value.integer_value.parse().unwrap();
            Ok((current_index + 1, timestamp))
        } else {
            // Here we can make sure we got unexpected data
            Err("DecodedAuditIndexFailed")
        }
    }

    /// Return data if success
    pub fn read<T: Decode>(&self, key: &[u8]) -> Result<Option<T>, &'static str> {
        let key = hex::encode(key);
        pink_extension::debug!("read: trying to read storage item, key: {}", key);
        let api = &format!("documents/index-storage/{key}");
        let response: HttpResponse = self.send_request("GET", api, None)?;
        if response.status_code == 404 {
            Ok(None)
        } else if let Ok(response) = pink_json::from_slice::<Document>(&response.body) {
            let data_str = response.fields.data.string_value.clone();
            let raw_data = hex::decode(data_str)
                .log_err("Get unexpected data format from database")
                .or(Err("InvalidDataStr"))?;
            let data: T = T::decode(&mut raw_data.as_slice())
                .log_err("Decode failed from data returned from database")
                .or(Err("DecodeDataFailed"))?;
            Ok(Some(data))
        } else {
            // Here we can make sure we got unexpected data
            Err("DecodedDataFailed")
        }
    }

    /// Create a new storage item
    pub fn insert(&self, key: &[u8], data: &[u8]) -> Result<(), &'static str> {
        let key: String = hex::encode(key);
        pink_extension::debug!("insert: trying to create storage item, key: {:?}", key);
        let data_str = hex::encode(data);
        let database = self.database.clone();
        let (audit_index, timestamp) = self.fetch_audit_index()?;
        let cmd = format!(
            r#"{{
                "writes": [
                    {{
                        "update": {{
                            "name": "{database}/documents/index-storage/{key}",
                            "fields": {{
                                "data": {{
                                    "stringValue": "{data_str}"
                                }}
                            }}
                        }},
                        "currentDocument": {{
                            "exists": false
                        }}
                    }},
                    {{
                        "update": {{
                            "name": "{database}/documents/index-audit/{audit_index}",
                            "fields": {{
                                "id": {{
                                    "stringValue": "{key}"
                                }},
                                "action": {{
                                    "stringValue": "insert"
                                }},
                                "data": {{
                                    "stringValue": "{data_str}"
                                }},
                                "timestamp": {{
                                    "timestampValue": "{timestamp}"
                                }}
                            }}
                        }},
                        "currentDocument": {{
                            "exists": false
                        }}
                    }},
                    {{
                        "update": {{
                            "name": "{database}/documents/index-storage/audit-index",
                            "fields": {{
                                "value": {{
                                    "integerValue": {audit_index}
                                }}
                            }}
                        }},
                    }}
                ]
            }}"#
        );
        let api = "documents:commit";
        let _ = self.send_request("POST", api, Some(&cmd))?;

        Ok(())
    }

    /// Update storage data
    pub fn update(&self, key: &[u8], data: &[u8]) -> Result<(), &'static str> {
        let key: String = hex::encode(key);
        pink_extension::debug!("update: trying to update storage item, key: {}", &key);
        let data_str = hex::encode(data);
        let database: String = self.database.clone();
        let (audit_index, timestamp) = self.fetch_audit_index()?;
        let cmd = format!(
            r#"{{
                "writes": [
                    {{
                        "update": {{
                            "name": "{database}/documents/index-storage/{key}",
                            "fields": {{
                                "data": {{
                                    "stringValue": "{data_str}"
                                }}
                            }}
                        }}
                    }},
                    {{
                        "update": {{
                            "name": "{database}/documents/index-audit/{audit_index}",
                            "fields": {{
                                "id": {{
                                    "stringValue": "{key}"
                                }},
                                "action": {{
                                    "stringValue": "update"
                                }},
                                "data": {{
                                    "stringValue": "{data_str}"
                                }},
                                "timestamp": {{
                                    "timestampValue": "{timestamp}"
                                }}
                            }}
                        }},
                        "currentDocument": {{
                            "exists": false
                        }}
                    }},
                    {{
                        "update": {{
                            "name": "{database}/documents/index-storage/audit-index",
                            "fields": {{
                                "value": {{
                                    "integerValue": {audit_index}
                                }}
                            }}
                        }},
                    }}
                ]
            }}"#
        );
        let api = "documents:commit";
        let _ = self.send_request("POST", api, Some(&cmd))?;

        Ok(())
    }

    /// Remove a document from remote storage
    pub fn delete(&self, key: &[u8]) -> Result<(), &'static str> {
        let key: String = hex::encode(key);
        let database: String = self.database.clone();
        let (audit_index, timestamp) = self.fetch_audit_index()?;
        let cmd = format!(
            r#"{{
                "writes": [
                    {{
                        "delete": "{database}/documents/index-storage/{key}"
                    }},
                    {{
                        "update": {{
                            "name": "{database}/documents/index-audit/{audit_index}",
                            "fields": {{
                                "id": {{
                                    "stringValue": "{key}"
                                }},
                                "action": {{
                                    "stringValue": "delete"
                                }},
                                "timestamp": {{
                                    "timestampValue": "{timestamp}"
                                }}
                            }}
                        }},
                        "currentDocument": {{
                            "exists": false
                        }}
                    }},
                    {{
                        "update": {{
                            "name": "{database}/documents/index-storage/audit-index",
                            "fields": {{
                                "value": {{
                                    "integerValue": {audit_index}
                                }}
                            }}
                        }},
                    }}
                ]
            }}"#
        );
        let api = "documents:commit";
        let _ = self.send_request("POST", api, Some(&cmd))?;
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
            amount: 0,
            fee: None,
            claim_nonce: None,
            claim_tx: None,
            merged_steps: vec![],
            execute_txs: vec![],
            execute_index: 0,
            sender: vec![],
            recipient: vec![],
            retry_counter: 0,
        };

        assert_eq!(client.read::<Task>(&task.id).unwrap(), None);
        // Save task to remote storage
        assert_eq!(client.insert(&task.id, &task.encode()), Ok(()));
        // Query storage for tasks
        let storage_task = client.read::<Task>(&task.id).unwrap().unwrap();
        assert_eq!(storage_task.encode(), task.encode());
        // Modify task status
        task.status = TaskStatus::Completed;
        // Update task data on remote storage
        assert_eq!(client.update(&task.id, &task.encode()), Ok(()));
        // Read again
        let updated_storage_task = client.read::<Task>(&task.id).unwrap().unwrap();
        assert_eq!(updated_storage_task.status, TaskStatus::Completed);
        // Delete task data from remote storage
        assert_eq!(client.delete(&updated_storage_task.id), Ok(()));
        // Verify task existence
        assert_eq!(client.read::<Task>(&task.id).unwrap(), None);
    }
}

use hyper::{Client, Uri};
use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::exit;
use std::thread::sleep;
use std::time::{Duration, SystemTime};
use tokio::runtime::Runtime;

#[derive(Debug, Clone)]
struct Message {
    role: String,
    content: String,
}

impl<'de> Deserialize<'de> for Message {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let map = HashMap::<String, String>::deserialize(deserializer)?;

        let (role, content) = match map.into_iter().next() {
            Some((role, content)) => (role, content),
            None => {
                return Err(serde::de::Error::custom(
                    "Message must have exactly one key-value pair",
                ));
            }
        };
        Ok(Message { role, content })
    }
}

impl Serialize for Message {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = HashMap::new();
        map.insert(self.role.clone(), self.content.clone());
        map.serialize(serializer)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Options {
    seed: Option<u32>,
    temperature: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Chat {
    model: String,
    options: Option<Options>,
    messages: Vec<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    address: String,
    wipe: Vec<String>,
    chat: Chat,
}

fn process_config(config: &Config) -> Result<Config, String> {
    let request_body = serde_json::to_string(&config.chat)
        .map_err(|error| format!("Can not form request body: {}", error))?;
    dbg!(&request_body);

    let async_runtime =
        Runtime::new().map_err(|error| format!("Can not create async runtime: {}", error))?;
    let response_body = async_runtime.block_on(async {
        let client = Client::new();

        let uri = format!("http://{}/api/chat", config.address)
            .as_str()
            .parse::<Uri>()
            .map_err(|error| format!("Can not form correct URI: {}", error))?;
        let request = hyper::Request::builder()
            .method(hyper::Method::POST)
            .uri(uri)
            .header("Content-Type", "application/json")
            .body(hyper::Body::from(request_body))
            .map_err(|error| format!("Can not attach body: {}", error))?;

        let response = client
            .request(request)
            .await
            .map_err(|error| format!("Can not send request: {}", error))?;
        let status = response.status();
        if !status.is_success() {
            return Err(format!("Invalid response status code: {}", status));
        }

        let response_body = hyper::body::to_bytes(response.into_body())
            .await
            .map_err(|error| format!("Can not get body of response: {}", error))?;
        let response_body_text = String::from_utf8(response_body.to_vec())
            .map_err(|error| format!("Can not parse body of response as UTF-8 text: {}", error))?;
        Ok(response_body_text)
    })?;

    dbg!(&response_body);
    let mut new_message: Message = serde_json::from_str(response_body.as_str())
        .map_err(|error| format!("Can not parse response body {:?}: {}", response_body, error))?;
    for tag in &config.wipe {
        let pattern = Regex::new(format!(r"\n*<#{tag}>(?:.|\n)*?<\/#{tag}>\n*").as_str()).map_err(
            |error| {
                format!(
                    "Can not form pattern for wiping tag from new message: {}",
                    error
                )
            },
        )?;
        new_message.content = pattern.replace_all(&new_message.content, "").into_owned();
    }
    let mut new_config = config.clone();
    new_config.chat.messages.push(new_message);
    Ok(new_config)
}

fn main() {
    let configs_directory = match dirs::config_dir() {
        Some(configs_directory) => configs_directory,
        None => {
            eprint!("Can not find configs directory");
            exit(1);
        }
    };
    let watch_directory = configs_directory.join("cryama");
    println!(
        "Watching files with 'yml' extension in directory {}",
        watch_directory.display()
    );

    let mut config_path_to_last_processed_time: HashMap<PathBuf, SystemTime> = HashMap::new();

    loop {
        let directory_iterator = match fs::read_dir(&watch_directory) {
            Ok(directory_iterator) => directory_iterator,
            Err(error) => {
                eprint!(
                    "Can not read directory {}: {}",
                    watch_directory.display(),
                    error
                );
                continue;
            }
        };
        for entry_result in directory_iterator {
            let entry = match entry_result {
                Ok(entry) => entry,
                Err(error) => {
                    eprint!("Can not read directory entry: {error}");
                    continue;
                }
            };
            let config_path = entry.path();
            if !config_path.is_file() {
                continue;
            }
            let extension = match config_path.extension() {
                Some(extension) => extension,
                None => continue,
            };
            if extension != "yml" {
                continue;
            }
            if let Some(last_processed_time) = config_path_to_last_processed_time.get(&config_path)
            {
                let metadata = match fs::metadata(&config_path) {
                    Ok(metadata) => metadata,
                    Err(error) => {
                        eprint!(
                            "Can not get metadata for file at path {}: {}",
                            config_path.display(),
                            error
                        );
                        continue;
                    }
                };
                let last_modification_time = match metadata.modified() {
                    Ok(last_modification_time) => last_modification_time,
                    Err(error) => {
                        eprint!(
                            "Can not get last modification time for file at path {}: {}",
                            config_path.display(),
                            error
                        );
                        continue;
                    }
                };
                if last_processed_time >= &last_modification_time {
                    continue;
                }
            }
            let config_text = match fs::read_to_string(&config_path) {
                Ok(config_text) => config_text,
                Err(error) => {
                    eprint!(
                        "Can not read file at path {}: {}",
                        config_path.display(),
                        error
                    );
                    continue;
                }
            };
            let mut config: Config = match serde_yaml::from_str(config_text.as_str()) {
                Ok(config) => config,
                Err(error) => {
                    eprint!(
                        "Can not parse Config from file at path {}: {}",
                        config_path.display(),
                        error
                    );
                    continue;
                }
            };
            let mut last_message = match config.chat.messages.pop() {
                Some(last_message) => last_message,
                None => continue,
            };
            if last_message.role != "user" {
                continue;
            }
            let stripped_last_message_content = match last_message.content.strip_suffix("//") {
                Some(stripped_last_message_content) => stripped_last_message_content,
                None => continue,
            };
            last_message.content = stripped_last_message_content.to_string();
            config.chat.messages.push(last_message);
            println!("Processing config from {}...", config_path.display());
            let new_config = match process_config(&config) {
                Ok(new_config) => new_config,
                Err(error) => {
                    eprint!(
                        "Can not process config from file at path {}: {}",
                        config_path.display(),
                        error
                    );
                    let new_last_processed_time = SystemTime::now();
                    config_path_to_last_processed_time
                        .entry(config_path)
                        .and_modify(|last_processed_time| {
                            *last_processed_time = new_last_processed_time
                        })
                        .or_insert(new_last_processed_time);
                    continue;
                }
            };
            let new_config_text = match serde_yaml::to_string(&new_config) {
                Ok(new_config_text) => new_config_text,
                Err(error) => {
                    eprint!(
                        "Can not serialize processed config from file at path {}: {}",
                        config_path.display(),
                        error
                    );
                    continue;
                }
            };
            match fs::write(&config_path, new_config_text) {
                Ok(()) => {}
                Err(error) => {
                    eprint!(
                        "Can not write processed config from file at path {}: {}",
                        config_path.display(),
                        error
                    );
                    continue;
                }
            };
            println!("Processed config from {}", config_path.display());
            let new_last_processed_time = SystemTime::now();
            config_path_to_last_processed_time
                .entry(config_path)
                .and_modify(|last_processed_time| *last_processed_time = new_last_processed_time)
                .or_insert(new_last_processed_time);
        }
        sleep(Duration::from_millis(200));
    }
}

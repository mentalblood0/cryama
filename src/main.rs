use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::process::exit;
use std::thread::sleep;
use std::time::{Duration, SystemTime};

mod http;

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
        let (role, content) = map.into_iter().next().ok_or(serde::de::Error::custom(
            "Message must have exactly one key-value pair",
        ))?;
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
    host: String,
    port: u16,
    #[serde(default)]
    wipe: Vec<String>,
    chat: Chat,
}

#[derive(Debug, Clone, Serialize)]
struct RequestMessage {
    role: String,
    content: String,
}

#[derive(Debug, Clone, Serialize)]
struct Request {
    model: String,
    stream: bool,
    options: Option<Options>,
    messages: Vec<RequestMessage>,
}

impl From<&Chat> for Request {
    fn from(chat: &Chat) -> Self {
        Request {
            model: chat.model.clone(),
            stream: false,
            options: chat.options.clone(),
            messages: {
                let mut result: Vec<RequestMessage> = Vec::new();
                for message in &chat.messages {
                    let request_message = RequestMessage {
                        role: message.role.clone(),
                        content: message.content.clone(),
                    };
                    result.push(request_message);
                }
                result
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct ResponseMessage {
    content: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Response {
    message: ResponseMessage,
}

fn process_config(config: &Config, client: &http::Client) -> Result<String, String> {
    let request = Request::from(&config.chat);
    let request_body = serde_json::to_string(&request)
        .map_err(|error| format!("Can not form request body: {error}"))?;

    let response = client.send_request(
        "POST",
        config.host.as_str(),
        config.port,
        "/api/chat",
        "application/json",
        request_body.as_str(),
    )?;
    let parsed_response: Response = serde_json::from_slice(&response.body)
        .map_err(|error| format!("Can not parse response body {:?}: {error}", response.body))?;

    let mut new_message_content = parsed_response.message.content;
    for tag in &config.wipe {
        let escaped_tag = regex::escape(tag);
        let pattern =
            Regex::new(format!(r"\n*<{escaped_tag}>(?:.|\n)*?<\/{escaped_tag}>\n*").as_str())
                .map_err(|error| {
                    format!("Can not form pattern for wiping tag from new message: {error}")
                })?;
        new_message_content = pattern.replace_all(&new_message_content, "").into_owned();
    }
    Ok(new_message_content)
}

fn main() {
    let configs_directory = match dirs::config_dir() {
        Some(configs_directory) => configs_directory,
        None => {
            eprintln!("Can not find configs directory");
            exit(1);
        }
    };
    let watch_directory = configs_directory.join("cryama");
    println!(
        "Watching files with 'yml' extension in directory {}",
        watch_directory.display()
    );

    let client = http::Client {
        response_status_line_regex: Regex::new(r"^HTTP\/(\d\.\d)\s+(\d{3})\s+(.*)$")
            .expect("Invalid regular expression for HTTP response first line"),
        response_header_line_regex: Regex::new(r"^([A-Za-z0-9-]+):\s*(.*)$")
            .expect("Invalid regular expression for HTTP response header line"),
    };

    let mut config_path_to_last_processed_time: HashMap<PathBuf, SystemTime> = HashMap::new();

    loop {
        let directory_iterator = match fs::read_dir(&watch_directory) {
            Ok(directory_iterator) => directory_iterator,
            Err(error) => {
                eprintln!(
                    "Can not read directory {}: {error}",
                    watch_directory.display(),
                );
                continue;
            }
        };
        for entry_result in directory_iterator {
            let entry = match entry_result {
                Ok(entry) => entry,
                Err(error) => {
                    eprintln!("Can not read directory entry: {error}");
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
                        eprintln!(
                            "Can not get metadata for file at path {}: {error}",
                            config_path.display(),
                        );
                        continue;
                    }
                };
                let last_modification_time = match metadata.modified() {
                    Ok(last_modification_time) => last_modification_time,
                    Err(error) => {
                        eprintln!(
                            "Can not get last modification time for file at path {}: {error}",
                            config_path.display(),
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
                    eprintln!(
                        "Can not read file at path {}: {error}",
                        config_path.display(),
                    );
                    continue;
                }
            };
            let config: Config = match serde_saphyr::from_str(config_text.as_str()) {
                Ok(config) => config,
                Err(error) => {
                    eprintln!(
                        "Can not parse Config from file at path {}: {error}",
                        config_path.display(),
                    );
                    continue;
                }
            };
            match config.chat.messages.last() {
                Some(last_message) => {
                    if !(last_message.role == "user"
                        && last_message
                            .content
                            .chars()
                            .last()
                            .map_or(false, |c| c.is_ascii_punctuation()))
                    {
                        continue;
                    }
                }
                None => continue,
            };

            println!("Processing config from {}...", config_path.display());
            let new_message_content = match process_config(&config, &client) {
                Ok(new_config) => new_config,
                Err(error) => {
                    eprintln!(
                        "Can not process config from file at path {}: {error}",
                        config_path.display(),
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
            let additional_config_text = format!(
                "    - assistant: |-\n        {}",
                new_message_content.replace("\n", "\n        ")
            );
            let mut config_file = match OpenOptions::new()
                .append(true)
                .create(true)
                .open(&config_path)
            {
                Ok(file) => file,
                Err(error) => {
                    eprintln!(
                        "Can not open config file for write at path {}: {error}",
                        config_path.display()
                    );
                    continue;
                }
            };
            match config_file.write_all(additional_config_text.as_bytes()) {
                Ok(()) => {}
                Err(error) => {
                    eprintln!(
                        "Can not write processed config from file at path {}: {error}",
                        config_path.display(),
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

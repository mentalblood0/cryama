use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::fs;

#[derive(Debug)]
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

        if map.len() != 1 {
            return Err(serde::de::Error::custom(
                "Message must have exactly one key-value pair",
            ));
        }

        let (role, content) = map.into_iter().next().unwrap();
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

#[derive(Debug, Serialize, Deserialize)]
struct Options {
    seed: Option<u32>,
    temperature: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Chat {
    model: String,
    options: Option<Options>,
    messages: Vec<Message>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    address: String,
    wipe: Vec<String>,
    chat: Chat,
}

fn main() {
    let watch_directory = dirs::config_dir().unwrap().join("cryama");

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
        for entry in directory_iterator {
            let entry = entry.unwrap();
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let extension = match path.extension() {
                Some(extension) => extension,
                None => continue,
            };
            if extension != "yml" {
                continue;
            }
            let config_text = match fs::read_to_string(&path) {
                Ok(config_text) => config_text,
                Err(error) => {
                    eprint!("Can not read file at path {}: {}", path.display(), error);
                    continue;
                }
            };
            let config: Config = match serde_yaml::from_str(config_text.as_str()) {
                Ok(config) => config,
                Err(error) => {
                    eprint!(
                        "Can not parse Config from file at path {}: {}",
                        path.display(),
                        error
                    );
                    continue;
                }
            };
            dbg!(&config);
        }
        break;
    }
}

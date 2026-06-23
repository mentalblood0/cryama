# 🪸 cryama

[![build](https://github.com/mentalblood0/cryama/actions/workflows/build.yml/badge.svg)](https://github.com/mentalblood0/cryama/actions/workflows/build.yml)

File-based interface to ollama

## Usage

The program will watch for configuration files in the directory default for those for your OS. It will tell you where it is watching them for once launched

Example configuration file:

```yaml
host: localhost
port: 11434
wipe:
  - think
remember: Be accurate
rewrite:
  - providing more examples
  - simplifying
chat:
  model: granite3.3:2b
  options:
    seed: 0
    temperature: 1.0
  messages:
    - user: When it is more efficient to use B-Tree instead of binary tree?
```

When detect an unprocessed or changed file with user message ended with punctuation character in the end, the program will send corresponding request to ollama API and write result to the same file as received

### Features

- `wipe` field is a list of xml-like tags, the program will remove text wrapped with them from assistant answers
- `remember` field is a string the program will append at the end of the last message before sending messages to ollama
- `rewrite` field is a list of strings. When received result message, the program will append user message "Rewrite your last message `string`" to messages and repeat request to ollama API, doing so sequentially for each `string` in the `rewrite` list and keeping only last (may it be already edited or original) message on each iteration

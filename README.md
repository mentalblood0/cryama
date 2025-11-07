# 🪸 cryama

File-based interface to ollama

## Usage

Program will watch for configuration files in directory default for those for your OS. It will tell you where it is watching them for once launched

Example configuration file:

```yaml
host: localhost
port: 11434
wipe:
  - think
chat:
  model: magistral:24b
  options: null
  messages:
    - user: How do I implement B-Tree in C?//
```

When detect unprocessed or changed file with user message ended with '//' in the end, program will send correcponding request to ollama API and write result to the same file as received

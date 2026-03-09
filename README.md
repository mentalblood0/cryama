# 🪸 cryama

[![tests](https://github.com/mentalblood0/cryama/actions/workflows/tests.yml/badge.svg)](https://github.com/mentalblood0/cryama/actions/workflows/tests.yml)

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
  model: granite3.3:2b
  options: null
  messages:
    - user: When it is more efficient to use B-Tree instead of binary tree?
```

When detect unprocessed or changed file with user message ended with punctuation character in the end, program will send correcponding request to ollama API and write result to the same file as received

Example configuration file after processing:

```yaml
host: localhost
port: 11434
wipe:
  - think
remember: Be accurate
chat:
  model: granite3.3:2b
  options: null
  messages:
    - user: When it is more efficient to use B-Tree instead of binary tree?
    - assistant: |-
        B-Trees are more efficient than Binary Trees, particularly in scenarios involving large data sets or databases with a high degree of concurrency, due to their optimized structure and operations. Here's why:
        
        1. **Balanced Tree**: While both Binary Trees and B-Trees can be balanced, B-Trees are specifically designed to maintain balance more effectively for larger datasets. This means that they have lower search times compared to Binary Trees, especially in real-world applications where data is often not uniformly distributed.
        
        2. **Sequential Access**: B-Trees excel at sequential access, which is crucial for databases and file systems where data is frequently accessed sequentially (e.g., reading a book or navigating a directory). This efficiency comes from their ability to store multiple keys per node, thus reducing the number of nodes traversed in a single operation.
        
        3. **Concurrency**: B-Trees are better suited for concurrent access by multiple users. The tree structure allows for efficient concurrency control and lock management, minimizing contention and improving overall system performance. In contrast, Binary Trees can lead to higher contention during concurrent writes due to their linear structure.
        
        4. **Disk Access Optimization**: B-Trees are optimized for disk access, as they minimize the number of disk accesses required by keeping related data together on the same disk surface. This is especially beneficial for databases stored on hard drives or other slower storage devices.
        
        5. **Space Efficiency**: Although both trees can store large amounts of data, B-Trees are more space-efficient due to their ability to pack multiple keys per node. This leads to reduced memory usage and improved cache utilization.
        
        In summary, while Binary Trees have a simpler structure and are easier to understand, B-Trees offer significant performance advantages in terms of efficiency, scalability, and concurrency management for large datasets or systems requiring frequent sequential access.
```

### Features

- `wipe` field is list of xml-like tags, program will remove text wrapped with them from assistant answers
- `remember` field is `system` role message. Program will append it right before the last message before sending them to ollama

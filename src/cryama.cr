require "log"
require "yaml"
require "json"
require "system/user"
require "http/client"

module Cryama
  class Message
    include JSON::Serializable

    property role : String
    property content : String

    def initialize(@role, @content)
    end

    def to_yaml(yaml : YAML::Nodes::Builder)
      yaml.mapping do
        yaml.scalar role
        yaml.scalar content
      end
    end

    def initialize(ctx : YAML::ParseContext, node : YAML::Nodes::Node)
      node.raise "Expected one key-value pair mapping, not #{node.kind}" unless node.is_a?(YAML::Nodes::Mapping) && node.nodes.size == 2
      @role = String.new ctx, node.nodes[0]
      @content = String.new ctx, node.nodes[1]
    end
  end

  class Options
    include YAML::Serializable
    include JSON::Serializable

    property seed : UInt32 | Nil = nil
    property temperature : Float32 | Nil = nil

    def initialize(@seed, @temperature)
    end
  end

  class Chat
    include YAML::Serializable
    include JSON::Serializable

    getter model : String
    getter options : Cryama::Options | Nil = nil
    property messages : Array(Message) = [] of Message

    @[YAML::Field(ignore: true)]
    getter stream = false

    def initialize(@model, @messages, @options = nil)
    end
  end

  class Config
    include YAML::Serializable

    getter address : String
    property chat : Chat
    getter end_suffix : String | Nil = nil

    @[YAML::Field(ignore: true)]
    getter end_suffix_default = "//"

    def initialize(@address, @chat)
    end

    def ready?
      last = chat.messages.last
      (last.role != "assistant") && last.content.ends_with?(end_suffix || end_suffix_default)
    end

    def unready
      chat.messages.last.content = chat.messages.last.content.chomp(end_suffix || end_suffix_default)
    end
  end

  class App
    {% if flag?(:windows) %}
      @@configs_dir = Path.new("~", "AppData", "cryama", "config").expand(home: true)
    {% else %}
      @@configs_dir = Path.new("~", ".config", "cryama").expand(home: true)
    {% end %}

    def configs(&)
      Dir.glob(@@configs_dir / "*.yml") do |str|
        yield Path.new str
      end
    end

    def create_example
      example_path = @@configs_dir / "example.yml"
      File.open(example_path, "w") do |example|
        example.print (
          Config.new "127.0.0.1:11434",
            Chat.new "granite3.1-dense",
              [Message.new("system", "You are strange but smart crystal bird"), Message.new("user", "hello")],
              Options.new 123, 0.5
        ).to_yaml
      end
      Log.info { "Created #{example_path}, to trigger processing add '//' to last message end" }
    end

    def process(config : Config)
      message_json = JSON.parse(HTTP::Client.post("#{config.address}/api/chat", body: config.chat.to_json).body)["message"]
      config.chat.messages << Message.new message_json["role"].as_s, message_json["content"].as_s
    end

    def monitor
      modification_times = {} of Path => Time
      loop do
        configs do |path|
          file = File.new path
          if !modification_times.has_key?(path) || modification_times[path] < file.info.modification_time
            modification_times[path] = file.info.modification_time
            config = begin
              Config.from_yaml File.read path
            rescue ex : YAML::ParseException
              Log.warn { ex.message }
              next
            end
            next if !config.ready?
            config.unready
            Log.info { "Processing #{path.stem}" }
            process config
            File.write path, config.to_yaml
            Log.info { "Processed #{path.stem}" }
          end
        end
        sleep 200.milliseconds
      end
    end

    def run
      Dir.mkdir_p @@configs_dir
      need_example = true
      configs do
        need_example = false
        break
      end
      create_example if need_example

      Log.info { "Watching for YAML files in #{@@configs_dir}" }
      monitor
    end
  end
end

Cryama::App.new.run

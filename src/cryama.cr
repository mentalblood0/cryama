require "log"
require "yaml"
require "json"
require "system/user"
require "http/client"

module Cryama
  class Message
    include YAML::Serializable
    include JSON::Serializable

    property role : String
    property content : String

    def initialize(@role, @content)
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
      chat.messages.last.content.ends_with?(end_suffix || end_suffix_default)
    end

    def unready
      chat.messages.last.content = chat.messages.last.content.chomp(end_suffix || end_suffix_default)
    end
  end

  class App
    getter configs_dir : Path

    def initialize(@configs_dir = (Path.new.posix? ? Path.new("~", ".config", "cryama") : Path.new("~", "AppData", "cryama")).expand(home: true))
    end

    def configs(&)
      Dir.glob(configs_dir / "*.yml") do |str|
        yield Path.new str
      end
    end

    def create_example
      example_path = configs_dir / "example.yml"
      File.open(example_path, "w") do |example|
        example.print (
          Config.new "127.0.0.1:11434",
            Chat.new "model name",
              [Message.new("user", "hello")],
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
            end
            next if !config || !config.ready?
            config.unready
            Log.info { "Processing #{path.stem}" }
            process config
            File.write path, config.to_yaml
          end
        end
        sleep 200.milliseconds
      end
    end

    def run
      Dir.mkdir_p configs_dir
      need_example = true
      configs do
        need_example = false
        break
      end
      create_example if need_example

      Log.info { "Watching for YAML files in #{configs_dir}" }
      monitor
    end
  end
end

Cryama::App.new.run

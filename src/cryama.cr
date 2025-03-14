require "log"
require "yaml"
require "json"
require "system/user"

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
    property model : String
    property options : Cryama::Options | Nil = nil
    property messages : Array(Message) = [] of Message

    @[YAML::Field(ignore: true)]
    getter stream = false

    def initialize(@model, @messages, @options = nil)
    end
  end

  class Config
    include YAML::Serializable
    include JSON::Serializable
    property address : String
    property chat : Chat

    def initialize(@address, @chat)
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
              [Message.new("user", "hello\nnext line"), Message.new("model", "hello\nnext line")],
              Options.new 123, 0.5
        ).to_yaml
      end
      Log.info { "created #{example_path}" }
    end

    def monitor
      modification_times = {} of Path => Time
      loop do
        configs do |path|
          file = File.new path
          if !modification_times.has_key?(path) || modification_times[path] < file.info.modification_time
            config = Config.from_yaml File.new(path).gets_to_end
            next if !config.chat.messages.last.content.ends_with? "//"
            Log.info { "processing #{path.stem}" }
            modification_times[path] = file.info.modification_time
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

      Log.info { "watching for *.yml files in #{configs_dir}" }
      monitor
    end
  end
end

Cryama::App.new.run

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
    {% if flag?(:windows) %}
      @@dir = Path.new("~", "AppData", "cryama").expand(home: true)
    {% else %}
      @@dir = Path.new("~", ".config", "cryama").expand(home: true)
    {% end %}

    include YAML::Serializable

    getter address : String
    property chat : Chat

    @[YAML::Field(ignore: true)]
    property name : String = ""

    def initialize(@name, @address, @chat)
    end

    def self.unprocessed(time : Time?, &)
      Dir.glob(@@dir/"*.yml")
        .select { |path| !time || (File.info(path).modification_time > time) }
        .each do |path|
          result = begin
            Config.from_yaml File.new path
          rescue ex : YAML::ParseException
            Log.warn { ex.message }
            next
          end
          result.name = Path.new(path).stem
          yield result if result.ready?
        end
    end

    def self.exists?
      Dir.glob @@dir / "*.yml" { return true }
      false
    end

    def ready?
      last = chat.messages.last
      (last.role != "assistant") && last.content.ends_with? "//"
    end

    def unready
      chat.messages.last.content = chat.messages.last.content.chomp "//"
    end

    def <<(message : Message)
      @chat.messages << message
    end

    def save
      Dir.mkdir_p @@dir
      File.write @@dir / (@name + ".yml"), self.to_yaml
    end

    def self.example
      Config.new "example",
        "127.0.0.1:11434",
        Chat.new "granite3.1-dense",
          [Message.new("system", "You are strange but smart crystal bird"), Message.new("user", "hello")],
          Cryama::Options.new 123, 0.5
    end

    def self.help
      "Store configs at #{@@dir}"
    end
  end

  class App
    def process(config : Config)
      response = HTTP::Client.post("#{config.address}/api/chat", body: config.chat.to_json).body
      message_json = JSON.parse(response)["message"]
      config << Message.new message_json["role"].to_s, message_json["content"].to_s
    end

    def watch
      last_check = nil
      loop do
        Config.unprocessed last_check do |config|
          config.unready
          Log.info { "Processing #{config.name}" }
          process config
          config.save
          Log.info { "Processed #{config.name}" }
        end
        last_check = Time.utc
        sleep 200.milliseconds
      end
    end

    def run
      if !Config.exists?
        Config.example.save
        puts "#{Config.help}. Created example config, to trigger processing add \"//\" to last message end"
      else
        puts Config.help
      end
      watch
    end
  end
end

Cryama::App.new.run

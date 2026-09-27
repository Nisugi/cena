# frozen_string_literal: true

# Where Lich's engine meets the world outside it, answered by Hydra: the
# game (`Game.puts`), the player's screen (`respond`, `_respond`, a
# script's own `puts`) and Lich's log (plan/46 section 5). What a script
# reads of its character is copy.rb's. Everything else is Lich's own code, unchanged,
# in ../lich.
#
# Each replacement keeps the shape of the Lich method it replaces
# (reference: lib/games.rb, lib/global_defs.rb at the commit README.md
# names), so a script sees what it saw under Lich: its sends echoed as
# `[name]>line`, its `respond` in fixed width, `echo` as `[name: text]`.

module Hydra
  class << self
    # The connection the edges use, set once by the runner.
    attr_accessor :connection

    # Show the player `lines`. Never raises: a script that could not be
    # shown its own output goes on, as it would under Lich with no
    # frontend attached.
    def say(lines, mono:, kind: 'info')
      connection.call('say', { text: Array(lines).join("\n"), kind: kind, mono: mono })
    rescue Unanswered => e
      $stderr.puts "[hydra] say: #{e.message}"
    end

    # Markup a script wrote, as text: Hydra draws no script markup
    # (SCRIPTS.md, `say`), and never puts it among the game's lines.
    def plain(text)
      Lich::Common::XmlEntities.decode(text.to_s.gsub(/<[^>]*>/, ''))
    end

    # The lines of `first` and `messages`, as Lich's `respond` builds them.
    def lines_of(first, messages)
      text = +''
      (first.is_a?(Array) ? first.flatten : [first]).each { |line| text << "#{line.to_s.chomp}\r\n" }
      messages.flatten.each { |message| text << "#{message.to_s.chomp}\r\n" }
      lines = text.chomp("\r\n").split(/\r?\n/, -1)
      lines.empty? ? [''] : lines
    end

    # What the runner defines and does not answer yet, by `Class#method`,
    # with what to use instead: the checker (check.rb) points at each call.
    def not_yet
      @not_yet ||= {}
    end

    # Define `klass`'s `name` as not answered yet: it raises, naming itself
    # and `instead`.
    def unanswered(klass, name, instead)
      label = "#{klass.name.split('::').last}##{name}"
      not_yet[label] = instead
      klass.define_method(name) do |*|
        raise NotImplementedError, "#{label} is not answered by Hydra yet (plan/46): #{instead}"
      end
    end
  end

  # A script's `$stdout`: what it prints, a line at a time, to the player.
  # Lich points `$stdout` at the frontend (lib/main/main.rb), so a script's
  # own `puts` reaches the screen, markup and all.
  class Screen
    def initialize
      @pending = +''
      @lock = Mutex.new
    end

    def write(*parts)
      text = parts.join
      lines = @lock.synchronize do
        @pending << text
        *done, @pending = @pending.split("\n", -1)
        done
      end
      Hydra.say(lines.map { |line| Hydra.plain(line.chomp("\r")) }, mono: false) unless lines.empty?
      text.bytesize
    end

    def puts(*args)
      write("\n") if args.empty?
      args.flatten.each do |arg|
        text = arg.nil? ? '' : arg.to_s
        write(text.end_with?("\n") ? text : "#{text}\n")
      end
      nil
    end

    def print(*args)
      write(*args.map(&:to_s))
      nil
    end

    def <<(text)
      write(text.to_s)
      self
    end

    def flush
      rest = @lock.synchronize { @pending.slice!(0..) }
      Hydra.say(Hydra.plain(rest), mono: false) unless rest.empty?
      self
    end

    def sync = true
    def sync=(_on); end
    def tty? = false
    def fileno = nil
  end
end

module Lich
  # Lich's log (lib/lich.rb) goes to the runner's standard error, which
  # Hydra keeps.
  def self.log(message)
    $stderr.puts "[lich] #{message}"
  end

  module Common
    # The frontend, as Lich's code asks after it: Hydra draws what a runner
    # says, as a client of Wrayth's family would, and speaks no GSL. Lich's
    # own (lib/common/frontend.rb) is launchers and Windows bindings a runner
    # has no use for.
    module Frontend
      def self.client = 'stormfront'
      def self.supports_xml?(_frontend = nil) = true
      def self.supports_gsl?(_frontend = nil) = false
      def self.supports_streams?(_frontend = nil) = true
      def self.supports_mono?(_frontend = nil) = false
      def self.supports_room_window?(_frontend = nil) = true
    end
  end

  # `Lich::Messaging`'s colours, as the kinds Hydra draws a notice in
  # (lib/messaging.rb, `msg_format`, names which colour is which).
  module Messaging
    KINDS = {
      'error' => 'error', 'yellow' => 'error', 'bold' => 'error', 'monster' => 'error', 'creature' => 'error',
      'warn' => 'warn', 'orange' => 'warn', 'gold' => 'warn', 'thought' => 'warn',
      'debug' => 'debug'
    }.freeze

    # A message in its colour's kind; `debug` only when Lich's debug
    # messaging is on, as Lich's own `msg` has it.
    def self.msg(type = 'info', msg = '', encode: true)
      _ = encode
      return if type == 'debug' && [nil, false, 'false'].include?(Lich.debug_messaging)

      lines = Hydra.lines_of(msg, [])
      lines.each { |line| Script.new_script_output(line) }
      Hydra.say(lines.map { |line| Hydra.plain(line) }, mono: false, kind: KINDS.fetch(type.to_s, 'info'))
    end

    # A table, kept in its columns.
    def self.mono(msg, encode: false)
      _ = encode
      raise StandardError, 'Lich::Messaging.mono only works with String parameters!' unless msg.is_a?(String)

      lines = msg.split("\n")
      lines.each { |line| Script.new_script_output(line) }
      Hydra.say(lines, mono: true)
    end
  end
end

# The game, as Lich's `Game` (lib/games.rb) is to a script.
class Game
  class << self
    # A script's line, echoed to the player as `[name]>line` unless the
    # script is silenced, then sent. Lich's `Game.puts`, whose
    # `Script.current` is the pause checkpoint.
    def puts(str)
      script = Script.current
      name = script&.name || '(unknown script)'
      name = "custom/#{name}" if script&.file_name && script.custom?
      respond "[#{name}]#{$SEND_CHARACTER}#{str}\r\n" unless script&.silent
      _puts(str)
    end

    # A line to the game, or to Hydra when it starts with the command
    # symbol. What Hydra would not send is said, naming the script; nothing
    # is ever sent twice.
    def _puts(str)
      script = Script.current_without_pause
      line = str.to_s.delete_prefix('<c>').chomp
      if script.respond_to?(:execution_guard_active?) && script.execution_guard_active?
        script.check_execution_guard!(command: line.dup.freeze)
      end
      answer = Hydra.connection.call('send', { line: line })
      return true if %w[sent ran].include?(answer['outcome'])

      name = script&.name || '(unknown script)'
      why = case answer['outcome']
            when 'unknown' then 'Hydra has no such command'
            when 'lost' then 'no connection'
            else answer['why']
            end
      respond "[#{name}: not sent (#{why}): #{line}]"
      nil
    rescue Hydra::Unanswered => e
      Lich.log "error: _puts: #{e.message}"
      nil
    end
  end
end

# Lich's `respond`, the lines to the player in fixed width as Lich gives
# them to Wrayth; every script listening to script output hears them.
def respond(first = '', *messages)
  lines = Hydra.lines_of(first, messages)
  lines.each { |line| Script.new_script_output(line) }
  Hydra.say(lines, mono: true)
rescue StandardError => e
  Lich.log "error: respond: #{e}\n\t#{e.backtrace&.first}"
end

# Lich's `_respond`, which passes markup through to the frontend. Hydra
# draws no script markup, so its text is shown.
def _respond(first = '', *messages)
  lines = Hydra.lines_of(first, messages)
  lines.each { |line| Script.new_script_output(line) }
  Hydra.say(lines.map { |line| Hydra.plain(line) }, mono: false)
rescue StandardError => e
  Lich.log "error: _respond: #{e}\n\t#{e.backtrace&.first}"
end

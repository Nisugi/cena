# frozen_string_literal: true

# The runner's ear: `listen`, in a loop, from the position after the last
# event handled, each event handed to Lich's engine as Lich would have
# handed it (crates/cena-agent/SCRIPTS.md, `listen`), and the hooks'
# answers sent back (hooks.rb).

module Hydra
  class Listener
    # Streams whose lines Lich takes out before a script sees them: their
    # text is shown elsewhere, and speech arrives once more on the main
    # stream (lib/common/markup.rb, SUPPRESSED_STREAMS and SPELLS_STREAM).
    UNHEARD = %w[spellfront inv bounty society reserve speech talk spells].freeze

    # How long one `listen` waits for the first event, in milliseconds.
    WAIT_MS = 25_000

    # How long to wait before asking again after Hydra did not answer.
    RETRY = 1

    # How many `listen`s in a row Hydra may leave unanswered before the
    # runner takes it that Hydra is gone -- crashed, killed, or listening on
    # another port since it restarted, which this runner never learns -- and
    # ends, its scripts with it. A token Hydra no longer holds is refused
    # every time, and ends it the same way.
    GIVE_UP = 10

    def initialize(connection, copy)
      @connection = connection
      @copy = copy
      @since = 0
    end

    # Listen until Hydra says nothing more will come, or is gone.
    def run
      unanswered = 0
      loop do
        heard = begin
          @connection.call('listen', { since: @since, timeout_ms: WAIT_MS }, wait: (WAIT_MS / 1000) + 10)
        rescue Unanswered => e
          $stderr.puts "[hydra] listen: #{e.message}"
          unanswered += 1
          if unanswered >= GIVE_UP
            $stderr.puts "[hydra] Hydra is gone: #{GIVE_UP} listens in a row unanswered. The runner ends."
            return
          end
          sleep RETRY
          next
        end
        unanswered = 0
        heard['events'].each { |event| handle(event) }
        Hooks.answer_shown
        @since = heard['next']
        respond '--- Hydra: some game lines were missed while scripts were busy.' if heard['lagged']
        break if heard['closed']
      end
    end

    private

    def handle(event)
      case event['kind']
      when 'state' then @copy.apply(event['fields'])
      when 'prompt' then XMLData.prompted(event['time'], event['text'])
      when 'line' then line(event)
      when 'ended' then Runs.ended(event)
      when 'typed' then typed(event['line'])
      when 'input' then Hooks.input(event['asked'], event['line'])
      when 'lagged'
        respond "--- Hydra: #{event['missed']} of the game's events were missed on the way to scripts."
      end
    rescue StandardError => e
      Lich.log "error: #{event['kind']}: #{e}\n\t#{e.backtrace&.first(3)&.join("\n\t")}"
    end

    # A game line, to every script listening, as Lich's `strip_xml` would
    # have left it: blank lines and the suppressed streams never reach one.
    # Then, as Lich runs them after its scripts have the line, the display
    # hooks, on every line: Lich's see every chunk.
    def line(event)
      text = event['text'].to_s
      unless UNHEARD.include?(event['stream'].to_s.downcase) || text.strip.empty?
        Script.new_downstream(text)
      end
      Hooks.shown(event['cursor'], text) if Hooks.display?
    end

    # A command the player typed for the runner, as Lich's `do_client`
    # runs one: its own command table first, then a script by that name.
    def typed(line)
      command = line.to_s.strip
      return if command.empty?
      return if Lich::Common::ClientCommands.dispatch(command)

      if command =~ /^([^\s]+)\s+(.+)/
        Script.start(Regexp.last_match(1), Regexp.last_match(2))
      else
        Script.start(command)
      end
    end
  end
end

# frozen_string_literal: true

# A script's hooks, answered for Hydra (plan/46 section 6.1;
# crates/cena-agent/SCRIPTS.md, Hooks).
#
# Lich's own DownstreamHook and UpstreamHook keep them, unchanged: a script
# adds one, names it, gives it a priority, and its death removes it as
# Lich's does. What Hydra adds is the asking. While a script has a display
# hook, what the player is shown of each game line waits for the hooks'
# answer; while one has an input hook, so does each line the player types.
# Hydra waits half a second, then goes on without the answer, so a slow
# hook costs a late line, never a lost one.
#
# The hooks see what a script sees: the game's text, not its markup, each
# line ending "\r\n" as a line of Lich's chunks does, and the player's
# typing as a Wrayth frontend sends it, `<c>` first. What they answer is
# shown as text: markup a hook adds is not drawn.

module Hydra
  module Hooks
    # Markup a hook may add to a line, as Lich's frontends would draw it:
    # tags only, so a line's own `<3` or `<--` is kept.
    TAG = %r{</?[A-Za-z][\w:-]*(?:\s[^<>]*)?/?>}

    @told = { display: false, input: false }
    @lock = Mutex.new
    @shown = []

    class << self
      # Tell Hydra which hooks there are, when that changed. Taken as told
      # before Hydra answers, so a line Hydra holds is never left
      # unanswered; untold again if Hydra did not hear it.
      def changed
        now = { display: !DownstreamHook.list.empty?, input: !UpstreamHook.list.empty? }
        @lock.synchronize do
          return if now == @told

          before = @told
          @told = now
          begin
            Hydra.connection.call('hooks', now)
          rescue Unanswered => e
            @told = before
            Lich.log "error: hooks: #{e.message}"
          end
        end
      end

      # Whether each game line is to be answered.
      def display?
        @told[:display] || !DownstreamHook.list.empty?
      end

      # The display hooks' answer about the game line at `cursor`, kept
      # until #answer_shown sends them together.
      def shown(cursor, text)
        result = DownstreamHook.run("#{text}\r\n")
        @shown << { cursor: cursor, text: result.nil? ? nil : as_shown(text, result) }
      end

      # Send the answers kept since the last.
      def answer_shown
        return if @shown.empty?

        lines = @shown
        @shown = []
        Hydra.connection.call('shown', { lines: lines })
      rescue Unanswered => e
        Lich.log "error: shown: #{e.message}"
      end

      # The input hooks' answer about the line the player typed, asked
      # as `asked`.
      def input(asked, line)
        result = UpstreamHook.run("<c>#{line}")
        answer = result.nil? ? nil : result.to_s.delete_prefix('<c>').lines.first.to_s.chomp
        Hydra.connection.call('input', { asked: asked, line: answer })
      rescue Unanswered => e
        Lich.log "error: input: #{e.message}"
      end

      private

      # What a hook answered, as the text to show: the line's own when the
      # hook left it, else without its markup.
      def as_shown(text, result)
        shown = result.to_s.sub(/\r?\n\z/, '')
        return text if shown == text

        Lich::Common::XmlEntities.decode(shown.gsub(TAG, ''))
      end
    end

    # The registries, told Hydra about as hooks come and go: added, removed,
    # or taken with a script's death.
    module Told
      def add(...)
        super.tap { Hooks.changed }
      end

      def remove(...)
        super.tap { Hooks.changed }
      end

      def cleanup_on_death(...)
        super.tap { Hooks.changed }
      end
    end
  end
end

Lich::Common::DownstreamHook.singleton_class.prepend(Hydra::Hooks::Told)
Lich::Common::UpstreamHook.singleton_class.prepend(Hydra::Hooks::Told)

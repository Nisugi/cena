# frozen_string_literal: true

# Hydra's built-ins, started as a Lich script starts Lich's by name
# (plan/46 section 7; crates/cena-agent/SCRIPTS.md, `perform`).
#
# The most-started Lich scripts are ones Hydra has built in: go2 first,
# 512 callers. A script that starts one by name -- `Script.run('go2',
# 'bank')`, `start_script('go2', [room])`, a `;go2` it has Lich run --
# starts Hydra's instead, **whatever file of that name the player has**
# (plan/46 section 10, question 5: the built-in wins). It runs as a Lich
# exec script named after it, so everything a script does with a script
# works on it: `Script.run` waits for it, `running?('go2')` sees it,
# `;k go2` and `kill_script` stop it, and stopping it stops Hydra's walk.

module Hydra
  # The runs this runner started, and how each ended, for whoever waits.
  module Runs
    @ended = {}
    @lock = Mutex.new
    @changed = ConditionVariable.new

    class << self
      # `listen` heard a run end.
      def ended(event)
        @lock.synchronize do
          @ended[event['run']] = event
          @changed.broadcast
        end
      end

      # How run `run` ended, once it has.
      def wait(run)
        @lock.synchronize do
          @changed.wait(@lock) until @ended.key?(run)
          @ended.delete(run)
        end
      end
    end
  end

  module Builtins
    # Lich's scripts Hydra has built in, by the name a script starts them
    # by, each with how its arguments become Hydra's command.
    COMMANDS = {
      'go2' => ->(args) { "go2 #{Builtins.destination(args)}" }
    }.freeze

    class << self
      # The built-in `Script.start`'s or `Script.run`'s arguments name, as
      # its name and arguments; nil when they name none.
      def named(args)
        name, rest = case args[0]
                     when Hash then [args[0][:name], args[0][:args]]
                     else [args[0], args[1]]
                     end
        rest = rest[:args] if rest.is_a?(Hash)
        rest = rest.join(' ') if rest.is_a?(Array)
        name = name.to_s.strip.downcase
        COMMANDS.key?(name) ? [name, rest.to_s] : nil
      end

      # Where to: go2's own settings (`_disable_confirm_`, `--delay=1`,
      # `typeahead=3`) are Lich's go2's, and Hydra's travel takes the
      # destination alone (reference/scripts/scripts/go2.lic, its settings).
      def destination(args)
        args.to_s.split.reject { |word| word =~ /\A_disable_confirm_\z/i || word.start_with?('--') || word =~ /\A[\w-]+=/ }
            .join(' ')
      end

      # Start built-in `name`, as an exec script named after it: the Script,
      # or false when one of that name already runs, as Lich answers a
      # second copy of a script.
      def start(name, args)
        if Script.running?(name)
          respond "--- Lich: #{name} is already running (use #{$lich_char}force [scriptname] if desired)."
          return false
        end
        line = COMMANDS.fetch(name).call(args)
        script = ExecScript.start("Hydra::Builtins.perform(#{line.inspect})", { quiet: true })
        script.instance_variable_set(:@name, name) if script
        script
      end

      # Have Hydra run `line`, and wait for it to end: whether the work was
      # done. A script killed while it waits stops Hydra's run with it.
      def perform(line)
        answer = Hydra.connection.call('perform', { line: line })
        if answer['refused']
          respond "[#{Script.current&.name}: #{answer['refused']}]"
          return false
        end
        run = answer['run']
        finished = false
        begin
          ended = Runs.wait(run)
          finished = true
          ended['work'] == 'completed'
        ensure
          stop(run) unless finished
        end
      end

      private

      def stop(run)
        Hydra.connection.call('stop', { run: run })
      rescue Unanswered => e
        Lich.log "error: stopping run #{run}: #{e.message}"
      end
    end
  end
end

# `Script.start`, `Script.run` and `Script.exists?` as a script calls them,
# a built-in's name answered by Hydra's (the Lich methods run for any other).
Lich::Common::Script.singleton_class.class_eval do
  alias_method :lich_start, :start
  alias_method :lich_run, :run
  alias_method :lich_exists?, :exists?

  def start(*args)
    found = Hydra::Builtins.named(args)
    found ? Hydra::Builtins.start(*found) : lich_start(*args)
  end

  # As Lich's: start it, then wait for it to end.
  def run(*args)
    found = Hydra::Builtins.named(args)
    return lich_run(*args) unless found

    script = Hydra::Builtins.start(*found)
    script&.join
  end

  def exists?(name)
    Hydra::Builtins::COMMANDS.key?(name.to_s.strip.downcase) || lich_exists?(name)
  end
end

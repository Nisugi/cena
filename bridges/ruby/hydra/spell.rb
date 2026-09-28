# frozen_string_literal: true

# Lich's `Spell`, answered from Hydra (plan/46 sections 4.3 and 11, steps 2
# and 9; crates/cena-agent/SCRIPTS.md, `spell`, `spells`): the spell table's
# row asked of Hydra once and kept; how long a cast lasts and what it costs
# asked each time, evaluated for the character as it is then; whether it is
# known and up read from the local copy. Lich's own (lib/common/spell.rb)
# evaluates the effect list's Ruby formulas in the runner; Hydra already
# evaluates the same table, so the runner carries neither the table nor the
# formulas.
#
# **Casting is Lich's own** (`cast` and the `force_` family, `lock_cast`,
# `after_stance`): the method as lib/common/spell.rb (BSD 3-Clause,
# ../lich/LICENSE.txt) writes it, the spell's `incant`, `stance`,
# `channel` and `cast_proc` from Hydra's table, sending and waiting as a
# script does. So what a script passes (`results_of_interest`, `channel` or
# `evoke`, `force_stance`) and what it is answered (the game's line) are
# Lich's. Two changes, both marked: a spell the table lacks (Lich's 9912 and
# its kin) is not up, rather than an error; and the Mental Acuity feat is
# not known while the feat list is unread, rather than an error.

module Hydra
  class Spell
    # Lich's (spell.rb): what `prepare` is answered with.
    PREPARE_REGEX = Regexp.union(
      /^You already have a spell readied!  You must RELEASE it if you wish to prepare another!$/,
      /^Your spell(?:song)? is ready\./,
      /^You can't think clearly enough to prepare a spell!$/,
      /^You are concentrating too intently .*?to prepare a spell\.$/,
      /^You are too injured to make that dextrous of a movement/,
      /^The searing pain in your throat makes that impossible/,
      /^But you don't have any mana!\.$/,
      /^You can't make that dextrous of a move!$/,
      /^As you begin to prepare the spell the wind blows small objects at you thwarting your attempt\.$/,
      /^You do not know that spell!$/,
      /^All you manage to do is cough up some blood\.$/,
      /^The incantations of countless spells swirl through your mind as a golden light flashes before your eyes\./
    )

    # Lich's (spell.rb): what a cast is answered with.
    RESULTS_REGEX = Regexp.union(
      /^(?:Cast|Sing) Roundtime [0-9]+ Seconds?\.$/,
      /^Cast at what\?$/,
      /^But you don't have any mana!$/,
      /^You don't have a spell prepared!$/,
      /keeps? the spell from working\./,
      /^Be at peace my child, there is no need for spells of war in here\.$/,
      /Spells of War cannot be cast/,
      /^As you focus on your magic, your vision swims with a swirling haze of crimson\.$/,
      /^Your magic fizzles ineffectually\.$/,
      /^All you manage to do is cough up some blood\.$/,
      /^And give yourself away!  Never!$/,
      /^You are unable to do that right now\.$/,
      /^You feel a sudden rush of power as you absorb [0-9]+ mana!$/,
      /^You are unable to drain it!$/,
      /leaving you casting at nothing but thin air!$/,
      /^You don't seem to be able to move to do that\.$/,
      /^Provoking a GameMaster is not such a good idea\.$/,
      /^You can't think clearly enough to prepare a spell!$/,
      /^You do not currently have a target\.$/,
      /The incantations of countless spells swirl through your mind as a golden light flashes before your eyes\./,
      /You can only evoke certain spells\./,
      /You can only channel certain spells for extra power\./,
      /That is not something you can prepare\./,
      /^\[Spell preparation time: \d seconds?\]$/,
      /^You are too injured to make that dextrous of a movement/,
      /^You can't make that dextrous of a move!$/
    )

    RELEASED = /^You feel the magic of your spell rush away from you\.$|^You don't have a prepared spell to release!$/

    @spells = {}
    @lock = Mutex.new
    @cast_lock = []
    @after_stance = nil

    class << self
      attr_accessor :after_stance
      attr_reader :cast_lock

      # A spell by number (215, '215') or name; nil when the table has none.
      def [](val)
        key = val.is_a?(Spell) ? val.num.to_s : val.to_s.strip
        @lock.synchronize { return @spells[key] if @spells.key?(key) }
        asked = key =~ /\A\d+\z/ ? { number: key.to_i } : { name: key }
        row = Hydra.connection.call('spell', asked)['spell']
        spell = row && new(row)
        @lock.synchronize do
          @spells[key] = spell
          @spells[spell.num.to_s] = spell if spell
        end
        spell
      rescue Unanswered => e
        Lich.log "error: Spell[#{val}]: #{e.message}"
        nil
      end

      # Every spell of the table, in order of number, made from one answer.
      def list
        @list ||= Array(Hydra.connection.call('spells', {})['spells']).map do |row|
          @lock.synchronize { @spells[row['number'].to_s] ||= new(row) }
        end
      rescue Unanswered => e
        Lich.log "error: Spell.list: #{e.message}"
        []
      end

      # Every spell up now that the table knows.
      def active
        now = Time.now
        XMLData.dialogs.values
               .flat_map { |list| list.select { |id, ends| id.is_a?(Integer) && ends > now }.keys }
               .uniq.filter_map { |number| self[number] }
      end

      def active?(val) = self[val]&.active? || false

      # Lich's (spell.rb): what a cast is answered with, and `results_of_interest` besides.
      def results_regex(results_of_interest: nil)
        return RESULTS_REGEX unless results_of_interest.is_a?(Regexp)

        Regexp.union(RESULTS_REGEX, results_of_interest)
      end

      # Lich's (spell.rb): one script casts at a time; the others wait
      # their turn, and a paused or ended one gives it up.
      def lock_cast
        script = Script.current
        @cast_lock.push(script)
        until (@cast_lock.first == script) or @cast_lock.empty?
          sleep 0.1
          Script.current # allows this loop to be paused
          @cast_lock.delete_if { |s| s.paused or not Script.list.include?(s) }
        end
      end

      def unlock_cast
        @cast_lock.delete(Script.current)
      end
    end

    attr_reader :num, :name, :type, :availability, :cast_proc, :no_incant, :last_cast
    attr_accessor :stance, :channel

    def initialize(row)
      @num = row['number']
      @name = row['name']
      @type = row['type']
      @availability = row['availability']
      @no_incant = row['incant'] == false
      @stance = row['stance'] || false
      @channel = row['channel'] || false
      @cast_proc = row['cast_proc']
    end

    # Lich's circle: the number's first digit, or first two past 999.
    def circle = (@num.to_s.length == 3 ? @num.to_s[0..0] : @num.to_s[0..1])

    def circle_name = Spells.get_circle_name(circle)
    alias circlename circle_name

    def incant? = !@no_incant

    def incant=(val)
      @no_incant = !val
    end

    # Known: the character's spell list names it. Before the list comes,
    # no spell is known, as Lich reads it.
    def known? = Array(XMLData.known_spells).include?(@num)

    # Lich's (spell.rb): known, and castable on whom `options` names.
    def available?(options = {})
      return false unless known?

      if options[:caster] and (options[:caster] !~ /^(?:self|#{XMLData.name})$/i)
        (options[:target] and (options[:target].downcase == options[:caster].downcase)) || @availability == 'all'
      elsif options[:target] and (options[:target] !~ /^(?:self|#{XMLData.name})$/i)
        @availability == 'all'
      else
        true
      end
    end

    def active? = timeleft.positive?

    # Minutes left, on this machine's clock; 0 when it is not up.
    def timeleft
      ends = XMLData.dialogs.values.filter_map { |list| list[@num] }.max
      return 0.0 unless ends

      [(ends - Time.now) / 60.0, 0.0].max
    end

    def minsleft = timeleft
    def secsleft = timeleft * 60
    def remaining = timeleft.as_time

    # Minutes one cast lasts, for the character now: on someone else when
    # `options[:target]` names someone, as Lich's `time_per` reads it.
    def time_per(options = {})
      target = options.is_a?(Hash) && options[:target] && options[:target] !~ /\A(?:self|#{XMLData.name})\z/i
      evaluated['minutes'][target ? 'target' : 'self'].to_f
    end

    def mana_cost(_options = {}) = cost('mana')
    def spirit_cost(_options = {}) = cost('spirit')
    def stamina_cost(_options = {}) = cost('stamina')
    def renew_cost(_options = {}) = cost('renew')

    # Whether the character has what one cast costs: Lich's checks of mana,
    # spirit (with one to spare) and stamina, without its Monk and
    # overexertion rules, which read what Hydra does not send yet.
    def affordable?(_options = {})
      Char.mana >= mana_cost && Char.spirit >= (spirit_cost.positive? ? spirit_cost + 1 : 0) &&
        Char.stamina >= stamina_cost
    end

    def to_s = @name

    def inspect = "#<Spell #{@num} #{@name}>"

    # Lich's (spell.rb), line for line but for the two changes the file's
    # header names: the spell cast, or prepared and cast at `target`; the
    # game's answer, or false when the character cannot afford it.
    def cast(target = nil, results_of_interest = nil, arg_options = nil, force_stance: nil)
      # fixme: find multicast in target and check mana for it
      check_energy = proc {
        if mental_acuity?
          unless (self.mana_cost <= 0) or Char.stamina >= (self.mana_cost * 2)
            echo 'cast: not enough stamina there, Monk!'
            sleep 0.1
            return false
          end
        else
          unless (self.mana_cost <= 0) or Char.mana >= self.mana_cost
            echo 'cast: not enough mana'
            sleep 0.1
            return false
          end
        end
        unless (self.spirit_cost <= 0) or Char.spirit >= (self.spirit_cost + 1 + [9912, 9913, 9914, 9916, 9916, 9916].delete_if { |num| !Spell.up?(num) }.length)
          echo 'cast: not enough spirit'
          sleep 0.1
          return false
        end
        unless (self.stamina_cost <= 0) or Char.stamina >= self.stamina_cost
          echo 'cast: not enough stamina'
          sleep 0.1
          return false
        end
      }
      script = Script.current
      if @type.nil?
        echo "cast: spell missing type (#{@name})"
        sleep 0.1
        return false
      end
      check_energy.call
      begin
        save_want_downstream = script.want_downstream
        save_want_downstream_xml = script.want_downstream_xml
        script.want_downstream = true
        script.want_downstream_xml = false
        Spell.cast_lock.push(script)
        until (Spell.cast_lock.first == script) or Spell.cast_lock.empty?
          sleep 0.1
          Script.current # allows this loop to be paused
          Spell.cast_lock.delete_if { |s| s.paused or not Script.list.include?(s) }
        end
        check_energy.call
        if @cast_proc
          waitrt?
          waitcastrt?
          check_energy.call
          begin
            proc { eval(@cast_proc) }.call
          rescue
            echo "cast: error: #{$!}"
            respond $!.backtrace[0..2]
            return false
          end
        else
          if @channel
            cast_cmd = 'channel'
          else
            cast_cmd = 'cast'
          end
          unless (arg_options.nil? || arg_options.empty?)
            if arg_options.split(" ")[0] =~ /incant|channel|evoke|cast/
              cast_cmd = arg_options.split(" ")[0]
              arg_options = arg_options.split(" ").drop(1)
              arg_options = arg_options.join(" ") unless arg_options.empty?
            end
          end

          if (((target.nil? || target.to_s.empty?) && !(@no_incant)) && (cast_cmd == "cast" && arg_options.nil?) || cast_cmd == "incant") && cast_cmd !~ /^(?:channel|evoke)/
            cast_cmd = "incant #{@num}"
          elsif (target.nil? or target.to_s.empty?) and (@type =~ /attack/i) and not [410, 435, 525, 912, 909, 609].include?(@num)
            cast_cmd += ' target'
          elsif target.is_a?(GameObj)
            cast_cmd += " ##{target.id}"
          elsif target.is_a?(Integer)
            cast_cmd += " ##{target}"
          elsif cast_cmd !~ /^incant/
            cast_cmd += " #{target}"
          end

          unless (arg_options.nil? || arg_options.empty?)
            cast_cmd += " #{arg_options}"
          end

          cast_result = nil
          loop {
            waitrt?
            if cast_cmd =~ /^incant/
              if (checkprep != @name) and (checkprep != 'None')
                dothistimeout 'release', 5, RELEASED
              end
            else
              unless checkprep == @name
                unless checkprep == 'None'
                  dothistimeout 'release', 5, RELEASED
                  unless (self.mana_cost <= 0) or Char.mana >= self.mana_cost
                    echo 'cast: not enough mana'
                    sleep 0.1
                    return false
                  end
                  unless (self.spirit_cost <= 0) or Char.spirit >= (self.spirit_cost + 1 + (if checkspell(9912) then 1 else 0 end) + (if checkspell(9913) then 1 else 0 end) + (if checkspell(9914) then 1 else 0 end) + (if checkspell(9916) then 5 else 0 end))
                    echo 'cast: not enough spirit'
                    sleep 0.1
                    return false
                  end
                  unless (self.stamina_cost <= 0) or Char.stamina >= self.stamina_cost
                    echo 'cast: not enough stamina'
                    sleep 0.1
                    return false
                  end
                end
                loop {
                  waitrt?
                  waitcastrt?
                  prepare_result = dothistimeout "prepare #{@num}", 8, PREPARE_REGEX
                  if prepare_result =~ /^Your spell(?:song)? is ready\./
                    break
                  elsif prepare_result == 'You already have a spell readied!  You must RELEASE it if you wish to prepare another!'
                    dothistimeout 'release', 5, RELEASED
                    unless (self.mana_cost <= 0) or Char.mana >= self.mana_cost
                      echo 'cast: not enough mana'
                      sleep 0.1
                      return false
                    end
                  elsif prepare_result =~ /^You can't think clearly enough to prepare a spell!$|^You are concentrating too intently .*?to prepare a spell\.$|^You are too injured to make that dextrous of a movement|^The searing pain in your throat makes that impossible|^But you don't have any mana!\.$|^You can't make that dextrous of a move!$|^As you begin to prepare the spell the wind blows small objects at you thwarting your attempt\.$|^You do not know that spell!$|^All you manage to do is cough up some blood\.$|The incantations of countless spells swirl through your mind as a golden light flashes before your eyes\./
                    sleep 0.1
                    return prepare_result
                  end
                }
              end
            end
            waitcastrt?
            if ((@stance && force_stance != false) || force_stance == true) && Char.stance != 'offensive'
              put 'stance offensive'
            end
            merged_results_regex = Spell.results_regex(results_of_interest: results_of_interest)

            if Effects::Spells.active?("Armored Casting")
              merged_results_regex = Regexp.union(/^Roundtime: \d+ sec.$/, merged_results_regex)
            else
              merged_results_regex = Regexp.union(/^\[Spell Hindrance for/, merged_results_regex)
            end
            cast_result = dothistimeout cast_cmd, 5, merged_results_regex
            if cast_result == "You don't seem to be able to move to do that."
              100.times { break if clear.any? { |line| line =~ /^You regain control of your senses!$/ }; sleep 0.1 }
              cast_result = dothistimeout cast_cmd, 5, merged_results_regex
            end
            if cast_cmd =~ /^incant/i && cast_result =~ /^\[Spell preparation time: (\d) seconds?\]$/
              sleep(Regexp.last_match(1).to_i + 0.5)
              cast_result = dothistimeout cast_cmd, 5, merged_results_regex
            end
            if ((@stance && force_stance != false) || force_stance == true)
              restore_stance_after_cast
            end
            if cast_result =~ /^Cast at what\?$|^Be at peace my child, there is no need for spells of war in here\.$|^Provoking a GameMaster is not such a good idea\.$/
              dothistimeout 'release', 5, RELEASED
            end
            if cast_result =~ /You can only evoke certain spells\.|You can only channel certain spells for extra power\./
              echo "cast: can't evoke/channel #{@num}"
              cast_cmd = cast_cmd.gsub(/^(?:evoke|channel)/, "cast")
              next
            end
            break unless ((circle.to_i == 10) && (cast_result =~ /^\[Spell Hindrance for/))
          }
          cast_result
        end
      ensure
        @last_cast = Time.now
        script.want_downstream = save_want_downstream
        script.want_downstream_xml = save_want_downstream_xml
        Spell.cast_lock.delete(script)
      end
    end

    def force_cast(target = nil, arg_options = nil, results_of_interest = nil, force_stance: nil)
      arg_options = arg_options.nil? || arg_options.empty? ? 'cast' : "cast #{arg_options}"
      cast(target, results_of_interest, arg_options, force_stance: force_stance)
    end

    def force_channel(target = nil, arg_options = nil, results_of_interest = nil, force_stance: nil)
      arg_options = arg_options.nil? || arg_options.empty? ? 'channel' : "channel #{arg_options}"
      cast(target, results_of_interest, arg_options, force_stance: force_stance)
    end

    def force_evoke(target = nil, arg_options = nil, results_of_interest = nil, force_stance: nil)
      arg_options = arg_options.nil? || arg_options.empty? ? 'evoke' : "evoke #{arg_options}"
      cast(target, results_of_interest, arg_options, force_stance: force_stance)
    end

    def force_incant(arg_options = nil, results_of_interest = nil, force_stance: nil)
      arg_options = arg_options.nil? || arg_options.empty? ? 'incant' : "incant #{arg_options}"
      cast(nil, results_of_interest, arg_options, force_stance: force_stance)
    end

    # What is up is the game's to say (the effects in the local copy), so
    # a script that writes it has nothing to write to.
    %i[putup putdown timeleft=].each do |name|
      Hydra.unanswered(self, name, 'Hydra reads what is up from the game\'s own list')
    end

    # A spell by number, up now; one the table lacks is not up.
    def self.up?(number) = self[number]&.active? || false

    private

    # The Mental Acuity feat, known: not while the feat list is unread.
    def mental_acuity? = (Lich::Gemstone::Infomon.get('feat.mental_acuity') || 0) >= 1

    # Lich's (spell.rb): after a stance spell, the stance the script asked
    # for (`Spell.after_stance`), or the safest the game allows.
    def restore_stance_after_cast
      after = Spell.after_stance.to_s.strip
      if !after.empty?
        Lich::Gemstone::Stance.change(after)
      elsif Char.stance !~ /^guarded$|^defensive$/
        Lich::Gemstone::Stance.change(Lich::Gemstone::Stance.safest)
      end
    rescue ArgumentError => e
      echo "cast: #{e.message}"
    end

    def evaluated
      Hydra.connection.call('spell', { number: @num })['spell'] || { 'minutes' => {}, 'costs' => {} }
    end

    def cost(kind) = evaluated['costs'][kind].to_f
  end
end

Spell = Hydra::Spell

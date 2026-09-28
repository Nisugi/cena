# frozen_string_literal: true

# The runner's local copy of its character (plan/46 section 4.2;
# crates/cena-agent/SCRIPTS.md, The local copy): Hydra's `state` fields,
# applied in order; `XMLData` answered from them as Lich's parser
# (lib/common/xmlparser.rb) answers from the game's XML; and `GameObj`,
# Lich's own class, filled from them as that parser fills it.
#
# Hydra names things its own way; the Lich names are made here, from one
# place: the room title bracketed, the exits spelled out, the statuses as
# Lich's indicators, the body parts all present, the effects as Lich's
# dialogs.

module Hydra
  # The copy: the fields of every `state` so far, the last word winning.
  class Copy
    def initialize
      @fields = {}.freeze
      @lock = Mutex.new
    end

    # Apply one `state` event's fields, and refill Lich's `GameObj` from
    # what changed. A reader sees the copy before or after, never between.
    def apply(changed)
      @lock.synchronize { @fields = @fields.merge(changed).freeze }
      refill(changed)
    end

    def [](key)
      @fields[key]
    end

    private

    def refill(changed)
      if changed.key?('room')
        room = changed['room'] || {}
        GameObj.begin_room_objs
        Array(room['creatures']).each do |npc|
          GameObj.new_npc(npc['id'], npc['noun'], npc['name'], Copy.status(npc))
        end
        Array(room['objects']).each { |obj| GameObj.new_loot(obj['id'], obj['noun'], obj['name']) }
        GameObj.commit_room_objs
        GameObj.begin_room_players
        Array(room['players']).each { |pc| GameObj.new_pc(pc['id'], pc['noun'], pc['name'], pc['status']) }
        GameObj.commit_room_players
      end
      return unless (hands = changed['hands'])

      Copy.hand(hands['right']) { |id, noun, name| GameObj.new_right_hand(id, noun, name) }
      Copy.hand(hands['left']) { |id, noun, name| GameObj.new_left_hand(id, noun, name) }
    end

    # A creature's status as Lich's is a word: `dead`, or what Hydra names
    # of it, or nil when nothing is wrong with it.
    def self.status(npc)
      return 'dead' if npc['dead']

      statuses = Array(npc['statuses'])
      statuses.empty? ? nil : statuses.join(' ')
    end

    # A hand as Lich's parser sets one: an item, or `Empty` with no id; a
    # hand nobody has described is left as it was.
    def self.hand(hand)
      case hand&.fetch('state', nil)
      when 'holding' then yield hand['id'], hand['noun'], hand['name']
      when 'empty' then yield nil, nil, 'Empty'
      end
    end
  end

  # Lich's `XMLData`, answered from the copy: its readers, by the names
  # lib/common/xmlparser.rb gives them. What Hydra does not send yet is not
  # answered, so a script that reads it fails naming it, rather than reading
  # a nil it cannot tell from the game's.
  class Data
    # Lich's names for the games (lib/common/xmlparser.rb reads them from
    # `<settingsInfo instance=>`); Hydra passes the code the login used.
    GAMES = { 'GS3' => 'GSIV', 'GS4' => 'GSIV', 'GSX' => 'GSPlat', 'GS4X' => 'GSPlat' }.freeze

    # Every body part Lich's parser starts at nothing.
    PARTS = %w[back leftHand rightHand head rightArm abdomen leftEye leftArm chest rightLeg neck
               leftLeg nsys rightEye].freeze

    # How long Lich takes an effect with no end to last (xmlparser.rb).
    DECADE = 10 * 31_536_000

    attr_reader :game, :name, :server_time, :server_time_offset, :prompt

    def initialize(game, name, copy)
      @game = GAMES.fetch(game.to_s.upcase, game.to_s.upcase)
      @name = name
      @copy = copy
      @server_time = 0
      @server_time_offset = 0.0
      @prompt = nil
    end

    # The game's prompt: its clock sets how far this machine's is from the
    # game's, as Lich's parser does at every prompt.
    def prompted(time, text)
      if time
        @server_time = time
        @server_time_offset = Time.now.to_f - time
      end
      @prompt = text
    end

    %w[health mana spirit stamina].each do |bar|
      define_method(bar) { vital(bar, 'current') }
      define_method("max_#{bar}") { vital(bar, 'max') }
    end

    def stance_text = @copy['stance']
    def stance_value = @copy['stance_percent'].to_i
    def mind_text = @copy['mind']
    def mind_value = @copy['mind_percent'].to_i
    def encumbrance_text = @copy['encumbrance']
    def encumbrance_full_text = @copy['encumbrance']
    def encumbrance_value = @copy['encumbrance_percent'].to_i
    def prepared_spell = @copy['prepared_spell'] || 'None'
    def roundtime_end = timer('roundtime')
    def cast_roundtime_end = timer('cast_roundtime')
    def current_target_id = @copy['target']
    # The map's room Hydra names, never guessed (map.rb, `Room.current`).
    def map_room = @copy['map_room']
    # The spells the spell list names, by number (spell.rb, `known?`).
    def known_spells = @copy['known_spells']
    # The sheet by Lich's Infomon keys (infomon.rb).
    def infomon = @copy.infomon

    # What the injury window shows, by Lich's numbers: 0 wounds, 1 scars,
    # 2 both; 0 until the game has said, as Lich starts.
    def injury_mode = { 'scars' => 1, 'both' => 2 }.fetch(@copy['injury_mode'], 0)

    # Level and experience, at Lich's defaults: 0, or nil for what is sent
    # only while it applies (lumnis, rpa, fashlonae).
    def level = experience['level'].to_i
    def exp = experience['experience'].to_i
    def next_level_text = experience['next_level'].to_s
    def next_level_value = experience['next_level_percent'].to_i
    def until_next = experience['until_next'].to_i
    def field_exp = experience['field_experience'].to_i
    def max_field_exp = experience['field_experience_max'].to_i
    def ascension_exp = experience['ascension_experience'].to_i
    def lumnis = experience['lumnis']
    def fashlonae = experience['fashlonae']
    def rpa = experience['rpa']&.to_f
    def current_target_ids = [@copy['target']].compact

    # The statuses as Lich's indicators: `IconSTUNNED` => `y`. One the game
    # has never reported is absent, as it is in Lich's.
    def indicator
      (@copy['statuses'] || {}).to_h { |id, on| ["Icon#{id.upcase}", on ? 'y' : 'n'] }
    end

    def injuries
      told = (@copy['injuries'] || {}).transform_values { |i| { 'wound' => i['wound'], 'scar' => i['scar'] } }
      PARTS.to_h { |part| [part, { 'wound' => 0, 'scar' => 0 }] }.merge(told)
    end

    def room_count = @copy['room_count'].to_i
    def room_description = @copy['room_description'].to_s
    def room_exits_string = @copy['room_exits_line'].to_s
    def room_id = room['id']&.to_i

    # `[Name]`, as Lich spells the title: the room number some players
    # show after it left off.
    def room_title
      title = room['title']
      title ? "[#{title.sub(/ - \d+$/, '')}]" : +''
    end

    # The exits, each spelled out as Lich's are: `north`, not `n`.
    def room_exits
      Array(room['exits']).map { |dir| LONGDIR[dir] || dir }
    end

    # The effects as Lich's dialogs: each list by its name, each effect
    # under its name and its id, with when it ends on this machine's clock.
    def dialogs
      Array(@copy['effects']).each_with_object({}) do |effect, dialogs|
        ends = if effect['ends_at']
                 Time.at(effect['ends_at'] + @server_time_offset)
               else
                 Time.now + DECADE
               end
        list = (dialogs[effect['category']] ||= {})
        list[effect['name']] = ends
        list[effect['id'] =~ /\A\d+\z/ ? effect['id'].to_i : effect['id']] = ends
      end
    end

    # Lich's own reading of its dialogs (xmlparser.rb, `active_spells`).
    def active_spells
      dialogs.sort.each_with_object({}) do |(list, effects), active|
        effects.each do |name, ends|
          next unless name.instance_of?(String)

          case list
          when /Active Spells|Buffs/ then active[name] = ends
          when /Cooldowns/ then active[name =~ /Recovery/ ? name : "#{name} Cooldown"] = ends
          when /Debuffs/ then active[name == 'Silenced' ? 'Silence' : name] = ends
          end
        end
      end
    end

    private

    def room = @copy['room'] || {}

    def experience = @copy['experience'] || {}

    def vital(bar, part)
      (((@copy['vitals'] || {})[bar]) || {})[part].to_i
    end

    def timer(name)
      (@copy[name] || {})['ends_at'].to_i
    end
  end
end

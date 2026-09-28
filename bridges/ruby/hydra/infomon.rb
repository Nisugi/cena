# frozen_string_literal: true

# Lich's `Infomon` (lib/gemstone/infomon.rb), answered from the local copy's
# sheet (plan/46 section 11, step 7; crates/cena-agent/SCRIPTS.md, The
# local copy). Lich keeps a SQLite store its parser fills from the game's
# text -- `info`, `skills`, `experience`, `society`, the PSM lists -- and
# its own `Stats`, `Skills`, `Spells`, `Society`, `Experience`,
# `Resources`, `Currency` and PSM classes read that store by key. Hydra's
# model reads the same text; its sheet is made into Lich's keys here, so
# those classes run unchanged.
#
# A value Hydra has not been told is nil, as a key Lich never stored.

module Hydra
  # Lich's Infomon keys, made from the copy's sheet.
  module Infomon
    # The six warcries, by Lich's key names for them (psms/warcry.rb).
    WARCRIES = %w[bellow yowlp growl shout cry holler].freeze

    # Lich's experience keys, by the sheet's names for them.
    EXPERIENCE = {
      'fame' => 'fame', 'field_experience' => 'field_experience_current',
      'field_experience_max' => 'field_experience_max', 'ascension_experience' => 'ascension_experience',
      'total_experience' => 'total_experience', 'deaths_sting' => 'deaths_sting',
      'long_term_experience' => 'long_term_experience', 'deeds' => 'deeds'
    }.freeze

    module_function

    # Lich's key spelling (infomon.rb, `_key`): `Two-Handed Weapons` and
    # `two_handed_weapons` are one key.
    def key(name) = name.to_s.downcase.tr(' -', '_').gsub(/_+/, '_')

    # Every key the sheet answers, by Lich's name, nil values left out.
    def keys(fields)
      keys = {}
      identity(keys, fields['identity'] || {})
      stats(keys, fields['stats'] || {})
      (fields['skills'] || {}).each do |name, skill|
        keys["skill.#{name}"] = skill['ranks']
        keys["skill.#{name}_bonus"] = skill['bonus']
      end
      (fields['circles'] || {}).each { |name, ranks| keys[key("spell.#{name}")] = ranks }
      (fields['psms'] || {}).each do |category, ranks|
        ranks.each { |mnemonic, rank| keys["#{category}.#{mnemonic}"] = rank }
      end
      known = Array(fields['warcries'])
      WARCRIES.each { |short| keys["warcry.#{short}"] = known.include?(short) ? 1 : 0 }
      standing(keys, fields)
      # The afflictions Lich reads from the game's text (infomon/status.rb),
      # which Hydra reads the same way into its statuses.
      (fields['statuses'] || {}).each { |name, on| keys["status.#{name}"] = on }
      (fields['currency'] || {}).each do |name, amount|
        keys[name == 'dust' ? 'currency.gemstone_dust' : "currency.#{name}"] = amount
      end
      experience = fields['experience'] || {}
      EXPERIENCE.each { |from, to| keys["experience.#{to}"] = experience[from] }
      keys['stat.experience'] = experience['experience']
      keys.compact.freeze
    end

    def identity(keys, identity)
      %w[race profession gender age].each { |name| keys["stat.#{name}"] = identity[name] }
      keys['account.type'] = identity['account']
    end

    # A stat by `info`'s columns, as Lich's parser stores them: its first
    # column bare, then `.enhanced` and `info full`'s `.base`.
    def stats(keys, stats)
      stats.each do |name, stat|
        { "stat.#{name}" => stat['ascended'], "stat.#{name}.enhanced" => stat['enhanced'],
          "stat.#{name}.base" => stat['base'] }.each do |at, column|
          next unless column

          keys[at] = column['value']
          keys["#{at}_bonus"] = column['bonus']
        end
      end
    end

    # The society and citizenship, `None` when the game said none, as
    # Lich's parser stores it; and the resources.
    def standing(keys, fields)
      if (society = fields['society'])
        keys['society.status'] = society['name'] || 'None'
        keys['society.rank'] = society['name'] ? society['rank'] : 0
      end
      if (citizenship = fields['citizenship'])
        keys['citizenship'] = citizenship['town'] || 'None'
      end
      resources = fields['resources'] || {}
      keys['resources.type'] = resources['kind']
      %w[weekly total suffused voln_favor covert_arts_charges shadow_essence].each do |name|
        keys["resources.#{name}"] = resources[name]
      end
    end
  end

  class Copy
    # The copy's sheet by Lich's Infomon keys, made again only when the
    # copy has changed since.
    def infomon
      fields = @fields
      return @infomon if @infomon_of.equal?(fields)

      table = Infomon.keys(fields)
      @infomon_of = fields
      @infomon = table
    end
  end
end

module Lich
  module Gemstone
    # Lich's Infomon, answered from the copy: its readers, by Lich's names.
    module Infomon
      def self._key(key) = Hydra::Infomon.key(key)

      def self.get(key) = XMLData.infomon[_key(key)]

      # Lich's reading of a flag: true, or 1.
      def self.get_bool(key)
        value = get(key)
        [true, false].include?(value) ? value : value == 1
      end

      # Whether the character's sheet wants reading again: while Hydra has
      # never been told its stats or skills. Lich asks it before a sync.
      def self.db_refresh_needed?
        %w[stat.strength skill.two_weapon_combat].any? { |key| get(key).nil? }
      end

      Hydra.unanswered(singleton_class, :sync, 'type info full, skills and the lists yourself: Hydra reads them')
    end
  end
end

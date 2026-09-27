# frozen_string_literal: true

# Lich's `Spell`, answered from Hydra (plan/46 section 4.3;
# crates/cena-agent/SCRIPTS.md, `spell`): the spell table's row asked of
# Hydra once and kept; how long a cast lasts and what it costs asked each
# time, evaluated for the character as it is then; whether it is known and
# up read from the local copy. Lich's own (lib/common/spell.rb) evaluates
# the effect list's Ruby formulas in the runner; Hydra already evaluates
# the same table, so the runner carries neither the table nor the
# formulas.
#
# What a script reads of a spell, by Lich's names. Not `cast`, which acts:
# a script that casts fails naming it, until Hydra's casting is reached
# through the runner.

module Hydra
  class Spell
    @spells = {}
    @lock = Mutex.new

    class << self
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

      # Every spell up now that the table knows.
      def active
        now = Time.now
        XMLData.dialogs.values
               .flat_map { |list| list.select { |id, ends| id.is_a?(Integer) && ends > now }.keys }
               .uniq.filter_map { |number| self[number] }
      end

      def active?(val) = self[val]&.active? || false
    end

    attr_reader :num, :name, :type, :availability

    def initialize(row)
      @num = row['number']
      @name = row['name']
      @type = row['type']
      @availability = row['availability']
    end

    # Lich's circle: the number's first digit, or first two past 999.
    def circle = (@num.to_s.length == 3 ? @num.to_s[0..0] : @num.to_s[0..1])

    # Known: the character's spell list names it. Before the list comes,
    # no spell is known, as Lich reads it.
    def known? = Array(XMLData.known_spells).include?(@num)

    def active? = timeleft.positive?

    # Minutes left, on this machine's clock; 0 when it is not up.
    def timeleft
      ends = XMLData.dialogs.values.filter_map { |list| list[@num] }.max
      return 0.0 unless ends

      [(ends - Time.now) / 60.0, 0.0].max
    end

    def secsleft = timeleft * 60

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

    Hydra.unanswered(self, :cast, 'cast with fput')

    private

    def evaluated
      Hydra.connection.call('spell', { number: @num })['spell'] || { 'minutes' => {}, 'costs' => {} }
    end

    def cost(kind) = evaluated['costs'][kind].to_f
  end
end

Spell = Hydra::Spell

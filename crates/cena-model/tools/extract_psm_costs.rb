#!/usr/bin/env ruby
# frozen_string_literal: true

#
# Extract what each PSM costs to use from Lich's PSM tables into the TSV that
# `cena-model` ships (`state/character/psm/cost.rs`).
#
# Run from the workspace root:
#
#   ruby crates/cena-model/tools/extract_psm_costs.rb \
#        reference/lich-5 \
#        crates/cena-model/data/psm_costs.tsv
#
# A COMMITTED tool, for the reason `extract_armaments.rb` gives: the table is
# re-cut whenever Lich changes it, and a script nobody kept makes that hand work.
#
# ## What is cut
#
# The six tables `PSMS.assess` reads a `:cost` from (`psms.rb:101-124`): combat
# maneuvers, feats, shield, weapon and armor specializations, and warcries.
# Ascension is left out: its costs are hard-coded 0 (`ascension.rb:13`), and
# `psm.rs` records why ascension is not a PSM here at all.
#
# Per row: the category, the mnemonic (`:short_name`, the key the game's own
# `cman list` column uses, VERIFIED in `psm.rs`), the long name (what the
# Cooldowns dialog calls it once normalized), the `:type` (the weapon and
# shield waivers in `weapon.rb:236,257-262` and `shield.rb:307` turn on it),
# the gauge and amount, and the source line.
#
# ## Why Ruby loads the files rather than a regex reading them
#
# `extract_armaments.rb`'s reason: a transcription made by regex acquires
# silent errors, and Ruby reads its own literals exactly. The stubs are two:
# `Lich::Util.normalize_lookup`, which two cost literals CALL while the hash is
# built, and the `PSMS` module the files reopen.
#
# ## The two costs that are not a number
#
# `burst_of_swiftness` and `surge_of_strength` cost 60 while their own
# cooldown is up and 30 otherwise (`cman.rb:62`, `:576`): the literal asks
# `normalize_lookup('Cooldowns', <long name>)`. Each file is loaded twice, once
# with that stub answering false and once true; a row whose cost differs
# between the two gets the second figure in `while_cooling`. A cost that
# depended on anything else would differ the same way and be caught by the
# assertion that the stub was asked about the row's own long name.
#
# ## Line numbers come from the text, and are checked against the load
#
# Ruby does not keep a hash entry's line. The text is scanned for each
# `"long_name" => {` and the `:cost` line that follows it, and the literal
# amount on that line must equal what the load produced, or this aborts.

src = ARGV[0] or abort 'usage: extract_psm_costs.rb <lich-5-dir> <out.tsv>'
out = ARGV[1] or abort 'usage: extract_psm_costs.rb <lich-5-dir> <out.tsv>'
dir = File.join(src, 'lib/gemstone/psms')

module Lich
  module Util
    class << self
      attr_accessor :cooling, :asked
    end

    def self.normalize_lookup(effect, val)
      (@asked ||= []) << [effect, val.to_s]
      @cooling ? true : false
    end
  end

  module Gemstone
    module PSMS; end
  end
end

# category, file, the module, its class variable.
TABLES = [
  ['armor', 'armor', 'Armor', :@@armor_techniques],
  ['cman', 'cman', 'CMan', :@@combat_mans],
  ['feat', 'feat', 'Feat', :@@feats],
  ['shield', 'shield', 'Shield', :@@shield_techniques],
  ['weapon', 'weapon', 'Weapon', :@@weapon_techniques],
  ['warcry', 'warcry', 'Warcry', :@@warcries]
].freeze

def load_tables(dir, cooling)
  Lich::Util.cooling = cooling
  Lich::Util.asked = []
  TABLES.to_h do |category, file, mod, var|
    # A second load re-assigns `feat.rb`'s constants; the warning is noise.
    verbose = $VERBOSE
    $VERBOSE = nil
    load File.join(dir, "#{file}.rb")
    $VERBOSE = verbose
    [category, Lich::Gemstone.const_get(mod).class_variable_get(var).dup]
  end
end

base = load_tables(dir, false)
cooling = load_tables(dir, true)
asked = Lich::Util.asked

commit = `git -C #{src} log -1 --format="%h %cs"`.strip

rows = []
TABLES.each do |category, file, _mod, _var|
  path = File.join(dir, "#{file}.rb")
  text = File.readlines(path)
  base[category].each do |long_name, psm|
    start = text.index { |l| l =~ /^\s*"#{Regexp.escape(long_name)}"\s*=>\s*\{/ } or
      abort "#{file}.rb: no entry line for #{long_name}"
    cost_at = (start...text.size).find { |i| text[i] =~ /:cost\s*=>/ } or
      abort "#{file}.rb: no :cost line after #{long_name}"
    abort "#{file}.rb: #{long_name} costs #{psm[:cost]} (one gauge expected)" unless psm[:cost].size == 1

    gauge, amount = psm[:cost].first
    hot = cooling[category].fetch(long_name)[:cost].fetch(gauge)
    if hot == amount
      literal = text[cost_at][/\{\s*#{gauge}:\s*(\d+)\s*\}/, 1] or
        abort "#{file}.rb:#{cost_at + 1}: #{long_name}'s cost is not a literal, yet did not change"
      abort "#{file}.rb:#{cost_at + 1}: text says #{literal}, load says #{amount}" unless literal.to_i == amount
      while_cooling = ''
    else
      abort "#{long_name}: its cost turns on something other than its own cooldown" unless
        asked.include?(['Cooldowns', long_name])
      while_cooling = hot.to_s
    end
    rows << [category, psm[:short_name], long_name, psm[:type], gauge, amount, while_cooling,
             "lich-5/lib/gemstone/psms/#{file}.rb:#{cost_at + 1}"]
  end
end

# Two feats share the mnemonic `wps` (`weighting` and `padding`, feat.rb:258-271).
# Both rows are kept, as Lich has them; what a lookup by mnemonic answers must
# not depend on which it finds, so rows sharing a key must agree on the cost.
rows.group_by { |r| r[0, 2] }.each do |key, same|
  next if same.map { |r| r[3, 4] }.uniq.size == 1

  abort "#{key.join(' ')}: rows sharing this key disagree: #{same.map { |r| r[2] }.join(', ')}"
end

File.open(out, 'w') do |f|
  f.puts '# Generated by tools/extract_psm_costs.rb from Lich\'s PSM tables.'
  f.puts '# Do not hand-edit: re-run the tool.'
  f.puts "# source\t#{dir.sub(%r{\A.*?(?=reference/)}, '')}"
  f.puts "# source-commit\t#{commit}"
  f.puts "# rows\t#{rows.size}"
  f.puts "# while_cooling: the cost while the PSM's own cooldown is up, when it differs (cman.rb:62,576)"
  f.puts %w[category mnemonic long_name type gauge cost while_cooling source].join("\t")
  rows.each { |r| f.puts r.join("\t") }
end
warn "#{rows.size} rows -> #{out}"

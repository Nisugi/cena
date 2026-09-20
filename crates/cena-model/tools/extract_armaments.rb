#!/usr/bin/env ruby
# frozen_string_literal: true

#
# Extract Lich's weapon, armor and shield stat tables into the TSVs that
# `cena-model` ships.
#
# Run from the workspace root:
#
#   ruby crates/cena-model/tools/extract_armaments.rb \
#        reference/lich-5/lib/gemstone/armaments \
#        crates/cena-model/data
#
# A COMMITTED tool, not a throwaway -- the same reasoning
# `extract_crit_tables.rb` records: the data gets re-cut whenever Lich updates
# its tables, and a script nobody kept means the next regeneration is hand
# work.
#
# ## Why Ruby rather than a Python parse
#
# These files are Ruby hash literals with symbol keys, nested hashes, arrays
# holding `nil` in fixed positions, and floats. Regex-parsing that is how a
# transcription acquires silent errors. Ruby reads its own literals exactly,
# and the harness is four stub methods -- `Lich::Util.deep_freeze` and the
# module nesting -- because the files do nothing but assign into class
# variables.
#
# ## Three shapes, three files
#
#   weapons.tsv   category, id, base_name, damage_factor, base_rt, min_rt,
#                 damage %s, AvD by armor sub-group
#   armor.tsv     armor group/sub-group, CvA, hindrance, training reqs
#   shields.tsv   size and evade modifiers by shield size
#   aliases.tsv   every alternate name -> its canonical id
#
# **The aliases are the point.** `broadsword` carries fifteen of them
# (`flyssa`, `katzbalger`, `spatha`, ...), and they are how a client turns
# `a flyssa` on the wire into "this is a broadsword with these stats". That
# knowledge lives only in this table -- the same argument `SKILL_NAME_MAP`
# makes for the 46 skill names.
#
# ## The 20-element arrays are POSITIONAL
#
# `hindrances` and `training_reqs` are indexed by spell circle, with `nil` in
# the positions that have no circle. Lich documents the mapping in a comment
# above each table; it is reproduced in `armor.tsv`'s header so the TSV is
# readable without the Ruby.
#
#   [0] Act Pen        [1] Minor Spiritual   [2] Major Spiritual  [3] Cleric
#   [4] Minor Elem     [5] Major Elemental   [6] Ranger           [7] Sorcerer
#   [8] Old Empath     [9] Wizard            [10] Bard            [11] Empath
#   [12] Minor Mental  [13] Major Mental     [14] Savant          [15] --
#   [16] Paladin       [17] Arcane           [18] --              [19] Lost Arts
#
# ## Two values are DERIVED, not stored, so they are not in the TSV
#
# `find_crit_divisor` and `find_coverage` (`armor_stats.rb:352-390`) are pure
# functions of the armor group: divisor is `{1=>5, 2=>6, 3=>7, 4=>9, 5=>11}`
# and coverage buckets ASG into four ranges. An earlier version of this script
# wrote empty columns for them, having guessed at field names the data does
# not have -- they belong in Rust, beside the data rather than in it.
#
# `nil` is written as an empty column rather than `0`: a circle that does not
# exist is not a circle with no hindrance, and collapsing them would make
# `armor.tsv` claim Savant armor penalties nobody has measured.

require 'fileutils'

src_dir = ARGV[0] or abort 'usage: extract_armaments.rb <armaments-dir> <out-dir>'
out_dir = ARGV[1] or abort 'usage: extract_armaments.rb <armaments-dir> <out-dir>'

# The harness. These files reference `Lich::Util.deep_freeze` and nothing else.
module Lich
  module Util
    def self.deep_freeze(obj) = obj
  end
end

# Load the sub-files first; `weapon_stats.rb` merges them by class variable.
Dir[File.join(src_dir, 'weapon_stats_*.rb')].sort.each { |f| require File.expand_path(f) }
require File.expand_path(File.join(src_dir, 'weapon_stats.rb'))
require File.expand_path(File.join(src_dir, 'armor_stats.rb'))
require File.expand_path(File.join(src_dir, 'shield_stats.rb'))

WS = Lich::Gemstone::Armaments::WeaponStats
AS = Lich::Gemstone::Armaments::ArmorStats
SS = Lich::Gemstone::Armaments::ShieldStats

weapons = WS.class_variable_get(:@@weapon_stats)
armor   = AS.class_variable_get(:@@armor_stats)
shields = SS.class_variable_get(:@@shield_stats)

FileUtils.mkdir_p(out_dir)

# ---------------------------------------------------------------------------
# weapons.tsv
# ---------------------------------------------------------------------------

# AvD is keyed by armor sub-group 1..20. Written as a single `|`-joined column
# rather than twenty columns: it is read as a unit and twenty mostly-identical
# headers would make the file unreadable.
ASG_RANGE = (1..20)

def num(value)
  case value
  when nil then ''
  when Float then format('%g', value)
  else value.to_s
  end
end

weapon_rows = []
alias_rows = []

weapons.each do |category, entries|
  entries.each do |id, w|
    dt = w[:damage_types] || {}
    avd = w[:avd_by_asg] || {}
    weapon_rows << [
      category, id, w[:base_name],
      # **`damage_factor` is an ARRAY indexed by armor group**, not a scalar:
      # `[nil, 0.4, 0.3, 0.23, 0.26, 0.18]` is DF against cloth, leather,
      # scale, chain and plate, with index 0 unused. Joined rather than
      # flattened into five columns for the same reason as AvD.
      (w[:damage_factor] || []).map { |d| num(d) }.join('|'),
      num(w[:base_rt]), num(w[:min_rt]),
      num(dt[:slash]), num(dt[:crush]), num(dt[:puncture]),
      (dt[:special] || []).join(','),
      ASG_RANGE.map { |asg| num(avd[asg]) }.join('|'),
      num(w[:weighting_type]), num(w[:weighting_amount])
    ]
    (w[:all_names] || []).each do |name|
      alias_rows << ['weapon', name, id, category]
    end
  end
end

# ---------------------------------------------------------------------------
# armor.tsv
# ---------------------------------------------------------------------------

HINDRANCE_SLOTS = 20

armor_rows = []
empty_subgroups = []
armor.each do |ag_key, subgroups|
  subgroups.each do |asg_key, a|
    # **Two sub-groups are declared and nil**: ag_1's asg_3 and asg_4. They
    # exist in the ASG numbering and hold no armor. Recorded in the TSV as a
    # row with a base_name of `-` rather than skipped, so a reader counting
    # sub-groups sees the gap is the game's rather than the extractor's.
    if a.nil?
      empty_subgroups << [ag_key, asg_key]
      armor_rows << ([''] * 2 + [ag_key.to_s.sub('ag_', ''), asg_key.to_s.sub('asg_', '')] + [''] * 8)
      armor_rows.last[1] = '-'
      next
    end
    armor_rows << [
      a[:type], a[:base_name],
      num(a[:armor_group]), num(a[:armor_sub_group]),
      num(a[:base_weight]), num(a[:min_rt]), num(a[:action_penalty]),
      num(a[:normal_cva]), num(a[:magical_cva]),
      (a[:hindrances] || Array.new(HINDRANCE_SLOTS)).map { |h| num(h) }.join('|'),
      num(a[:hindrance_max]),
      (a[:training_reqs] || Array.new(HINDRANCE_SLOTS)).map { |t| num(t) }.join('|')
    ]
    (a[:all_names] || []).each do |name|
      alias_rows << ['armor', name, a[:base_name], a[:type]]
    end
  end
end

# ---------------------------------------------------------------------------
# shields.tsv
# ---------------------------------------------------------------------------

shield_rows = []
shields.each do |id, sh|
  shield_rows << [
    id, sh[:category], sh[:base_name],
    num(sh[:size_modifier]), num(sh[:evade_modifier]), num(sh[:base_weight])
  ]
  (sh[:all_names] || []).each { |name| alias_rows << ['shield', name, id, sh[:category]] }
end

def write_tsv(path, header, rows)
  File.open(path, 'w') do |fh|
    fh.puts(header.join("\t"))
    rows.each { |r| fh.puts(r.join("\t")) }
  end
  warn "#{File.basename(path)}: #{rows.length} rows"
end

write_tsv(
  File.join(out_dir, 'weapons.tsv'),
  %w[category id base_name damage_factor_by_ag_0_to_5 base_rt min_rt slash crush puncture
     special avd_by_asg_1_to_20 weighting_type weighting_amount],
  weapon_rows.sort_by { |r| [r[0].to_s, r[1].to_s] }
)

warn "empty armor sub-groups: #{empty_subgroups.inspect}" unless empty_subgroups.empty?

write_tsv(
  File.join(out_dir, 'armor.tsv'),
  %w[type base_name armor_group armor_sub_group base_weight min_rt
     action_penalty normal_cva magical_cva hindrances_0_to_19 hindrance_max
     training_reqs_0_to_19],
  armor_rows.sort_by { |r| [r[2].to_s.rjust(3, '0'), r[3].to_s.rjust(3, '0')] }
)

write_tsv(
  File.join(out_dir, 'shields.tsv'),
  %w[id category base_name size_modifier evade_modifier base_weight],
  shield_rows
)

write_tsv(
  File.join(out_dir, 'armament_aliases.tsv'),
  %w[kind alias id category],
  alias_rows.sort_by { |r| [r[0], r[1].to_s.downcase] }
)

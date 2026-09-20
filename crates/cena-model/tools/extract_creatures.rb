#!/usr/bin/env ruby
# frozen_string_literal: true

#
# Extract Lich's 627 creature templates into the TSVs that `cena-model` ships.
#
# Run from the workspace root:
#
#   ruby crates/cena-model/tools/extract_creatures.rb \
#        reference/lich-5/lib/gemstone/creatures \
#        crates/cena-model/data
#
# A COMMITTED tool, for the reason `extract_armaments.rb` and
# `extract_crit_tables.rb` record: the data gets re-cut whenever Lich updates
# its bestiary, and a script nobody kept means the next regeneration is hand
# work. Lich's own files carry a `schema_version`, which is a standing promise
# that the shape WILL change.
#
# ## Why Ruby rather than a Python parse
#
# Each template is a Ruby hash literal, and the numeric fields are frequently
# **Ranges** (`(106..116)`), which no JSON or regex parse reads correctly.
# MEASURED across the 627: `melee` is a Range 511 times, an Integer 37 times
# and nil 79 times. Ruby reads its own literals exactly; the harness is a bare
# `eval` because these files are data and nothing else.
#
# ## Four files, because the data is four shapes
#
#   creatures.tsv        one row per creature: identity, level, hp, flags
#   creature_areas.tsv   where it is found, as room UID ranges
#   creature_attacks.tsv its attacks, with AS
#   creature_messages.tsv death / flee / arrival / decay lines
#
# One wide row per creature would need 1,394 area columns and a variable
# number of attack columns. These are four tables because they are four
# cardinalities.
#
# ## THE TRI-STATE IS THE POINT, AND IT IS WRITTEN AS THREE VALUES
#
# `_creature_template.rb` says so in as many words: *"true/false/nil if
# unknown"*, and for `limbs`, *"has no limbs left!' after repeated casts =
# true; a refusal on the very first cast = false"* -- these were measured in
# game, one creature at a time, and a blank means nobody has measured it.
#
# MEASURED, and the unknowns are not a rounding error:
#
#   sleepable   nil:342  false:158  true:127
#   limbs       true:328 nil:298    false:1
#   blood       true:320 nil:161    false:146
#
# So `sleepable` is UNKNOWN for 342 of 627. Collapsing nil to false would tell
# a sorcerer that 342 creatures resist Sleep when the truth is that nobody has
# tried. Written here as `true` / `false` / empty, and read into an
# `Option<bool>` on the other side.
#
# ## A range is written `lo..hi` and a scalar as itself
#
# Rather than two columns, because the reader has to distinguish "known to
# vary" from "known to be exactly this" and a lo==hi pair would erase it.
#
# ## Six AS values are malformed IN THE SOURCE and are kept
#
#   ashen_patrician_vampire     Rapier          as="566 to"
#   athletic_dark_eyed_incubus  Ensnare         as="(lunge) 245-276"
#   ethereal_triton_psionicist  Unarmed combat  as="390 UAF"
#   shining_winged_disir        Lance           as=""
#   spiked_cavern_urchin        Pincer (attack) as="(barbed spines) 176"
#   triton_brawler              UCS             as="414 UAF"
#
# Data-entry damage in Lich's bestiary, not a schema Cena should model. They
# are written to the `as_raw` column and left unparsed rather than dropped,
# per Rule 2.2 -- nothing is dropped without saying so.

require 'fileutils'

src_dir = ARGV[0] or abort 'usage: extract_creatures.rb <creatures-dir> <out-dir>'
out_dir = ARGV[1] or abort 'usage: extract_creatures.rb <creatures-dir> <out-dir>'

FileUtils.mkdir_p(out_dir)

# `_creature_template.rb` is the commented schema, not a creature.
files = Dir[File.join(src_dir, '*.rb')]
        .reject { |f| File.basename(f).start_with?('_') }
        .sort

# A tri-state flag: true, false, or empty for "nobody has measured this".
def tri(value)
  case value
  when true then 'true'
  when false then 'false'
  else ''
  end
end

# A number that may be a Range, an Integer, or absent.
def num_or_range(value)
  case value
  when Range then "#{value.first}..#{value.last}"
  when Numeric then value.to_s
  else ''
  end
end

def clean(text)
  # Tabs and newlines would break the TSV; messaging text contains neither in
  # practice, but a single stray one would silently shift every later column.
  text.to_s.gsub(/[\t\r\n]+/, ' ').strip
end

creature_rows = []
area_rows     = []
attack_rows   = []
message_rows  = []
unparsed_as   = []

files.each do |path|
  id = File.basename(path, '.rb')
  data = eval(File.read(path), binding, path) # rubocop:disable Security/Eval
  defense = data[:defense_attributes] || {}
  treasure = data[:treasure] || {}

  creature_rows << [
    id, clean(data[:name]), clean(data[:noun]),
    data[:level], num_or_range(data[:max_hp]),
    clean(data[:family]), clean(data[:type]), clean(data[:size]),
    data[:height], data[:speed],
    tri(data[:undead]), tri(data[:blood]), tri(data[:bones]), tri(data[:limbs]),
    tri(data[:witherable]), tri(data[:sympathy]), tri(data[:muggable]),
    tri(data[:sleepable]), tri(data[:bcs]),
    data[:boss] ? 'true' : 'false', clean(data[:boss_type]),
    clean(defense[:asg]),
    num_or_range(defense[:melee]), num_or_range(defense[:ranged]),
    num_or_range(defense[:bolt]), num_or_range(defense[:udf]),
    # The eight profession TDs. Written individually because a caster wants
    # their own circle's number, not an average.
    num_or_range(defense[:bar_td]), num_or_range(defense[:cle_td]),
    num_or_range(defense[:emp_td]), num_or_range(defense[:pal_td]),
    num_or_range(defense[:ran_td]), num_or_range(defense[:sor_td]),
    num_or_range(defense[:wiz_td]), num_or_range(defense[:mjs_td]),
    clean(treasure[:skin]),
    treasure[:coins] ? 'true' : 'false',
    treasure[:boxes] ? 'true' : 'false',
    treasure[:gems] ? 'true' : 'false'
  ]

  (data[:areas] || []).each do |area|
    (area[:uids] || []).each do |uid|
      lo, hi = uid.is_a?(Range) ? [uid.first, uid.last] : [uid, uid]
      area_rows << [id, clean(area[:name]), lo, hi]
    end
  end

  (data.dig(:attack_attributes, :physical_attacks) || []).each do |attack|
    as = attack[:as]
    parsed = num_or_range(as)
    unparsed_as << [id, attack[:name], as] if parsed.empty? && !as.nil?
    # A malformed AS is preserved verbatim rather than dropped (Rule 2.2).
    # `shining_winged_disir`'s Lance has `as: ""` -- an EMPTY malformed value,
    # which would be indistinguishable from "no AS recorded" if written as an
    # empty column. Written as `?` so the reader can still tell the source said
    # something unusable from the source saying nothing.
    raw = if !parsed.empty? then ''
          elsif as.nil? then ''
          elsif clean(as).empty? then '?'
          else clean(as)
          end
    attack_rows << [id, clean(attack[:name]), parsed, raw, clean(attack[:damage_type])]
  end

  messaging = data[:messaging] || {}
  # Death and flee are what a combat consumer matches on; description is what a
  # player reads. `attacks` and `info` are deliberately NOT extracted: they are
  # nested per-attack and per-profession structures that nothing reads yet, and
  # a table with no consumer is a table that rots (Rule -1).
  %i[death flee arrival decay].each do |kind|
    Array(messaging[kind]).each do |line|
      message_rows << [id, kind.to_s, clean(line)]
    end
  end
end

def write_tsv(path, header, rows)
  File.open(path, 'w') do |fh|
    fh.puts(header.join("\t"))
    rows.each { |r| fh.puts(r.join("\t")) }
  end
  warn "#{File.basename(path)}: #{rows.length} rows"
end

write_tsv(
  File.join(out_dir, 'creatures.tsv'),
  %w[id name noun level max_hp family type size height speed
     undead blood bones limbs witherable sympathy muggable sleepable bcs
     boss boss_type asg melee ranged bolt udf
     bar_td cle_td emp_td pal_td ran_td sor_td wiz_td mjs_td
     skin coins boxes gems],
  creature_rows
)

write_tsv(
  File.join(out_dir, 'creature_areas.tsv'),
  %w[creature_id area uid_lo uid_hi],
  area_rows
)

write_tsv(
  File.join(out_dir, 'creature_attacks.tsv'),
  %w[creature_id name as as_raw damage_type],
  attack_rows
)

write_tsv(
  File.join(out_dir, 'creature_messages.tsv'),
  %w[creature_id kind text],
  message_rows
)

warn "creatures: #{files.length}"
unless unparsed_as.empty?
  warn "AS values kept verbatim because they are malformed in the source (#{unparsed_as.length}):"
  unparsed_as.each { |id, name, as| warn "  #{id}: #{name.inspect} as=#{as.inspect}" }
end

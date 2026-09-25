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
# ## EVERYTHING the template says is extracted, and an unknown key is fatal
#
# > **AUTHOR, 2026-09-24:** *"The reason it was all there was multiple
# > reasons, one of which is a comprehensive beastiary, the other is the
# > messages have uses just because they haven't been made apparent yet. For
# > example when get to implementing kswole's behaviors that requires their
# > casting prep line."*
#
# Until 2026-09-24 this tool kept 4 of the 13 message kinds (death, flee,
# arrival, decay) and dropped attack messaging on the grounds that "a table
# with no consumer is a table that rots (Rule -1)". That was wrong twice over.
# Rule -1 is about abstractions, not about what a port keeps; and a table this
# tool regenerates from upstream cannot rot. It also dropped `description`
# with no reason at all, four of the twelve TD columns, every attack category
# but physical, the defenses, abilities, equipment and `otherclass`, and it
# wrote an UNKNOWN treasure flag as `false` -- the collapse this header's own
# tri-state section forbids. MEASURED: of 7,125 message lines it kept 3,862,
# and none of the 50 `info` tips.
#
# So every path in the source is now either written or refused. `KNOWN` below
# is the schema this tool reads; a template carrying anything else aborts the
# run with the path named, and a field that is always empty today
# (`transmogs`, `alchemy`, `abilities_misc`) aborts the day it is not. An
# upstream addition cannot vanish silently again.
#
# ## Seven files, because the data is seven shapes
#
#   creatures.tsv          one row per creature: identity, flags, defenses,
#                          treasure. Read by HEADER NAME on the Rust side, so a
#                          new column is not an index shift.
#   creature_areas.tsv     where it is found, as room UID ranges
#   creature_attacks.tsv   every attack, by category: physical, bolt_spell,
#                          warding_spell, offensive_spell, maneuver,
#                          special_ability. `as` is attack strength; warding
#                          spells carry `cs`, casting strength, instead.
#   creature_messages.tsv  every message kind, with a KEY: the attack name for
#                          `attacks`, the effect for `triggers`, empty for the
#                          flat kinds. The key is what tells a matched line
#                          *which* attack or *which* special it was.
#   creature_info.tsv      the per-profession and general tips (`info`)
#   creature_lists.tsv     the plain lists: otherclass, equipment, immunities,
#                          defensive spells and abilities, special defenses,
#                          special notes, and treasure's armaments and other
#   creature_abilities.tsv the typed abilities, with their effects
#
# One wide row per creature would need 1,394 area columns and a variable
# number of everything else. These are seven tables because they are seven
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
#   boxes       true:357 nil:164    false:106
#
# Written as `true` / `false` / empty, and read into an `Option<bool>` on the
# other side. The treasure flags were not, until 2026-09-24.
#
# `skin` is four-valued in the source: a name (307), nil (314), `false` (5, it
# does not skin) and `true` (1, it skins into something nobody wrote down). So
# it is two columns: `skin`, the name, and `skins`, the tri-state.
#
# ## A range is written `lo..hi` and a scalar as itself
#
# Rather than two columns, because the reader has to distinguish "known to
# vary" from "known to be exactly this" and a lo==hi pair would erase it.
#
# ## An empty string is skipped, and counted
#
# 32 descriptions and one trigger are `""` in the source: a list with nothing
# in it yet. Such a string gets no row -- the loader would refuse a message
# with no text -- and every run prints how many it skipped, so a jump in that
# number is seen.
#
# ## Malformed strengths are kept verbatim
#
# Fourteen strengths are malformed IN THE SOURCE -- six physical AS values,
# and eight in the spell categories, which the first port did not read:
#
#   ashen_patrician_vampire           Rapier                  as="566 to"
#   athletic_dark_eyed_incubus        Ensnare                 as="(lunge) 245-276"
#   ethereal_triton_psionicist        Unarmed combat          as="390 UAF"
#   shining_winged_disir              Lance                   as=""
#   spiked_cavern_urchin              Pincer (attack)         as="(barbed spines) 176"
#   triton_brawler                    UCS                     as="414 UAF"
#   battle_worn_empyrean_captain      ??? (bolt)              as="???"
#   battle_worn_empyrean_captain      ??? (warding)           cs="???"
#   fulminating_stormborn_primordial  Cone of Elements (518)  as="???"
#   fulminating_stormborn_primordial  Elemental Strike (415)  cs="???"
#   kiramon_defender                  Disease (warding)       cs="Poison"
#   pallid_fog_cloaked_kelpie         Major Cold (907)        as="???"
#   pallid_fog_cloaked_kelpie         Elemental Strike (415)  cs="???"
#   shining_winged_disir              Censure (warding)       cs=""
#
# Data-entry damage in Lich's bestiary, not a schema Cena should model. They
# are written to the `_raw` column and left unparsed rather than dropped,
# per Rule 2.2 -- nothing is dropped without saying so. An EMPTY malformed
# value is written `?`, so the reader can tell "the source said something
# unusable" from "the source said nothing".

require 'fileutils'
require 'set'

src_dir = ARGV[0] or abort 'usage: extract_creatures.rb <creatures-dir> <out-dir>'
out_dir = ARGV[1] or abort 'usage: extract_creatures.rb <creatures-dir> <out-dir>'

FileUtils.mkdir_p(out_dir)

# `_creature_template.rb` is the commented schema, not a creature.
files = Dir[File.join(src_dir, '*.rb')]
        .reject { |f| File.basename(f).start_with?('_') }
        .sort

# Every path this tool reads. Keys that are names rather than schema -- an
# attack's name under `messaging.attacks`, a trigger's effect, a profession
# under `class_tips`, an ability's effect flag -- are collapsed to `<key>`.
KNOWN = Set.new(%w[
  schema_version name noun url picture level family type undead blood bones
  limbs witherable sympathy muggable sleepable boss boss_type bcs max_hp
  speed height size special_other
  otherclass otherclass[] equipment equipment[] alchemy abilities_misc
  areas areas[] areas[].name areas[].uids areas[].uids[]
  attack_attributes
  attack_attributes.physical_attacks attack_attributes.physical_attacks[]
  attack_attributes.physical_attacks[].name attack_attributes.physical_attacks[].as
  attack_attributes.bolt_spells attack_attributes.bolt_spells[]
  attack_attributes.bolt_spells[].name attack_attributes.bolt_spells[].as
  attack_attributes.warding_spells attack_attributes.warding_spells[]
  attack_attributes.warding_spells[].name attack_attributes.warding_spells[].cs
  attack_attributes.offensive_spells attack_attributes.offensive_spells[]
  attack_attributes.offensive_spells[].name
  attack_attributes.maneuvers attack_attributes.maneuvers[]
  attack_attributes.maneuvers[].name
  attack_attributes.special_abilities attack_attributes.special_abilities[]
  attack_attributes.special_abilities[].name attack_attributes.special_abilities[].note
  attack_attributes.special_abilities[].type
  attack_attributes.special_notes attack_attributes.special_notes[]
  defense_attributes defense_attributes.asg defense_attributes.melee
  defense_attributes.ranged defense_attributes.bolt defense_attributes.udf
  defense_attributes.bar_td defense_attributes.cle_td defense_attributes.emp_td
  defense_attributes.pal_td defense_attributes.ran_td defense_attributes.sor_td
  defense_attributes.wiz_td defense_attributes.mje_td defense_attributes.mne_td
  defense_attributes.mjs_td defense_attributes.mns_td defense_attributes.mnm_td
  defense_attributes.immunities defense_attributes.immunities[]
  defense_attributes.defensive_spells defense_attributes.defensive_spells[]
  defense_attributes.defensive_spells[].name
  defense_attributes.defensive_abilities defense_attributes.defensive_abilities[]
  defense_attributes.defensive_abilities[].name defense_attributes.defensive_abilities[].note
  defense_attributes.special_defenses defense_attributes.special_defenses[]
  abilities abilities[] abilities[].id abilities[].name abilities[].type
  abilities[].target abilities[].typical_duration_s abilities[].dispellable
  abilities[].notes abilities[].effects abilities[].effects.<key>
  treasure treasure.coins treasure.magic_items treasure.gems treasure.boxes
  treasure.skin treasure.other treasure.other[] treasure.armaments
  treasure.armaments[] treasure.transmogs treasure.blunt_required
  messaging messaging.description messaging.description[]
  messaging.arrival messaging.arrival[] messaging.flee messaging.flee[]
  messaging.death messaging.death[] messaging.decay messaging.decay[]
  messaging.search messaging.search[] messaging.spell_prep messaging.spell_prep[]
  messaging.stand messaging.stand[] messaging.stun_break messaging.stun_break[]
  messaging.ambient messaging.ambient[]
  messaging.attacks messaging.attacks.<key> messaging.attacks.<key>[]
  messaging.triggers messaging.triggers.<key> messaging.triggers.<key>[]
  messaging.info messaging.info.general messaging.info.general[]
  messaging.info.miscellany messaging.info.miscellany[]
  messaging.info.class_tips messaging.info.class_tips.<key>
  messaging.info.class_tips.<key>[]
]).freeze

# Fields that exist and are empty in every template today. Their shape when
# filled is unknown, so a value is a schema change and stops the run.
ALWAYS_EMPTY = %w[alchemy abilities_misc treasure.transmogs].freeze

# The flat message kinds, in Lich's spelling.
FLAT_MESSAGES = %i[death flee arrival decay description search spell_prep
                   stand stun_break ambient].freeze

# Every path in one template, with name-like keys collapsed.
def paths(value, path = '', out = [])
  out << path unless path.empty?
  case value
  when Hash
    value.each do |key, inner|
      segment = if path.match?(/\A(messaging\.(attacks|triggers|info\.class_tips)|abilities\[\]\.effects)\z/)
                  '<key>'
                else
                  key.to_s
                end
      paths(inner, path.empty? ? segment : "#{path}.#{segment}", out)
    end
  when Array
    value.each { |inner| paths(inner, "#{path}[]", out) }
  end
  out
end

def empty?(value)
  value.nil? || value == [] || value == {} || value == ''
end

# A tri-state flag: true, false, or empty for "nobody has measured this".
def tri(value)
  case value
  when true then 'true'
  when false then 'false'
  when nil then ''
  else raise "not a flag: #{value.inspect}"
  end
end

# A number that may be a Range, an Integer, or absent. Anything else is
# returned as '' here and kept by `raw`.
def num_or_range(value)
  case value
  when Range then "#{value.first}..#{value.last}"
  when Numeric then value.to_s
  else ''
  end
end

# The verbatim form of a strength that did not parse; `?` for an empty one.
def raw(value)
  return '' if value.nil? || !num_or_range(value).empty?

  clean(value).empty? ? '?' : clean(value)
end

# Text for a TSV cell. A tab or a newline would break the row, so they are
# ESCAPED -- `\t`, `\n`, and `\\` for a backslash -- and `load.rs` unescapes
# them. Not flattened to a space, which is what this did until 2026-09-24:
# MEASURED, 26 descriptions and 13 tips are several paragraphs or a bulleted
# list, and 7 attack and trigger messages are TWO game lines (a warning, then
# the effect). Flattened, the bullets ran together and a two-line message
# became one line the game never prints.
def clean(text)
  text.to_s.gsub("\r\n", "\n").strip
      .gsub('\\') { '\\\\' }.gsub("\n", '\\n').gsub("\t", '\\t')
end

# A field that is a list of strings, a single string, or absent.
def strings(value)
  case value
  when nil then []
  when String then [value]
  when Array then value
  else raise "not a string list: #{value.inspect}"
  end
end

creature_rows = []
area_rows     = []
attack_rows   = []
message_rows  = []
info_rows     = []
list_rows     = []
ability_rows  = []
unparsed      = []
empty_strings = 0
unknown       = Hash.new { |h, k| h[k] = [] }

files.each do |path|
  id = File.basename(path, '.rb')
  data = eval(File.read(path), binding, path) # rubocop:disable Security/Eval

  paths(data).uniq.each { |p| unknown[p] << id unless KNOWN.include?(p) }
  ALWAYS_EMPTY.each do |field|
    value = data.dig(*field.split('.').map(&:to_sym))
    unknown["#{field} (now filled)"] << id unless empty?(value)
  end

  defense = data[:defense_attributes] || {}
  treasure = data[:treasure] || {}
  attack = data[:attack_attributes] || {}
  messaging = data[:messaging] || {}
  skin = treasure[:skin]

  creature_rows << [
    id, clean(data[:name]), clean(data[:noun]),
    data[:level], num_or_range(data[:max_hp]),
    clean(data[:family]), clean(data[:type]), clean(data[:size]),
    data[:height], num_or_range(data[:speed]),
    tri(data[:undead]), tri(data[:blood]), tri(data[:bones]), tri(data[:limbs]),
    tri(data[:witherable]), tri(data[:sympathy]), tri(data[:muggable]),
    tri(data[:sleepable]), tri(data[:bcs]),
    data[:boss] ? 'true' : 'false', clean(data[:boss_type]),
    clean(defense[:asg]),
    num_or_range(defense[:melee]), num_or_range(defense[:ranged]),
    num_or_range(defense[:bolt]), num_or_range(defense[:udf]),
    # All twelve TDs: nine professions' own circles and the three minor and
    # major circles every hybrid casts from. Written individually because a
    # caster wants their own circle's number, not an average.
    *%i[bar_td cle_td emp_td pal_td ran_td sor_td wiz_td
        mje_td mne_td mjs_td mns_td mnm_td].map { |k| num_or_range(defense[k]) },
    skin.is_a?(String) ? clean(skin) : '',
    skin.is_a?(String) ? 'true' : tri(skin),
    tri(treasure[:coins]), tri(treasure[:boxes]), tri(treasure[:gems]),
    tri(treasure[:magic_items]), tri(treasure[:blunt_required]),
    clean(data[:special_other]), clean(data[:url]), clean(data[:picture]),
    data[:schema_version]
  ]

  (data[:areas] || []).each do |area|
    (area[:uids] || []).each do |uid|
      lo, hi = uid.is_a?(Range) ? [uid.first, uid.last] : [uid, uid]
      area_rows << [id, clean(area[:name]), lo, hi]
    end
  end

  {
    physical: :physical_attacks, bolt_spell: :bolt_spells,
    warding_spell: :warding_spells, offensive_spell: :offensive_spells,
    maneuver: :maneuvers, special_ability: :special_abilities
  }.each do |category, key|
    (attack[key] || []).each do |entry|
      unparsed << [id, category, entry[:name], entry[:as]] if !entry[:as].nil? && num_or_range(entry[:as]).empty?
      unparsed << [id, category, entry[:name], entry[:cs]] if !entry[:cs].nil? && num_or_range(entry[:cs]).empty?
      attack_rows << [
        id, category, clean(entry[:name]),
        num_or_range(entry[:as]), raw(entry[:as]),
        num_or_range(entry[:cs]), raw(entry[:cs]),
        clean(entry[:note]), clean(entry[:type])
      ]
    end
  end

  # An empty string in a message or tip list says nothing -- MEASURED, 32
  # descriptions and one trigger are `""` -- so it gets no row, and the run
  # reports how many it skipped.
  keep = lambda do |text|
    empty_strings += 1 if clean(text).empty?
    !clean(text).empty?
  end
  FLAT_MESSAGES.each do |kind|
    strings(messaging[kind]).select(&keep).each { |line| message_rows << [id, kind, '', clean(line)] }
  end
  %i[attacks triggers].each do |kind|
    (messaging[kind] || {}).each do |key, lines|
      strings(lines).select(&keep).each { |line| message_rows << [id, kind, key, clean(line)] }
    end
  end

  info = messaging[:info] || {}
  %i[general miscellany].each do |section|
    strings(info[section]).select(&keep).each { |tip| info_rows << [id, section, clean(tip)] }
  end
  (info[:class_tips] || {}).each do |profession, tips|
    strings(tips).select(&keep).each { |tip| info_rows << [id, profession, clean(tip)] }
  end

  # A list entry is a string, or a {name:, note:} hash.
  {
    otherclass: data[:otherclass], equipment: data[:equipment],
    immunities: defense[:immunities], defensive_spells: defense[:defensive_spells],
    defensive_abilities: defense[:defensive_abilities],
    special_defenses: defense[:special_defenses],
    special_notes: attack[:special_notes],
    armaments: treasure[:armaments], treasure_other: treasure[:other]
  }.each do |list, value|
    Array(value.is_a?(String) ? [value] : value).each do |entry|
      name, note = entry.is_a?(Hash) ? [entry[:name], entry[:note]] : [entry, nil]
      list_rows << [id, list, clean(name), clean(note)]
    end
  end

  (data[:abilities] || []).each do |ability|
    # Effects are flags (`rooted: true`) or carry a value
    # (`blocks_spells_at_or_above: 50`): written `flag` or `flag=value`,
    # `;`-separated, in the source's order.
    effects = (ability[:effects] || {}).map do |flag, value|
      value == true ? flag.to_s : "#{flag}=#{clean(value)}"
    end
    ability_rows << [
      id, ability[:id], clean(ability[:name]), ability[:type], ability[:target],
      ability[:typical_duration_s], tri(ability[:dispellable]),
      effects.join(';'), clean(ability[:notes])
    ]
  end
end

unless unknown.empty?
  warn 'The source carries paths this extractor does not read. Extend KNOWN and'
  warn 'write them somewhere -- do not drop them:'
  unknown.sort.each { |p, ids| warn "  #{p}  (#{ids.length}: #{ids.first(3).join(', ')}...)" }
  abort 'extract_creatures.rb: refusing to write a partial bestiary'
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
     bar_td cle_td emp_td pal_td ran_td sor_td wiz_td
     mje_td mne_td mjs_td mns_td mnm_td
     skin skins coins boxes gems magic_items blunt_required
     special_other url picture schema_version],
  creature_rows
)

write_tsv(File.join(out_dir, 'creature_areas.tsv'), %w[creature_id area uid_lo uid_hi], area_rows)

write_tsv(
  File.join(out_dir, 'creature_attacks.tsv'),
  %w[creature_id category name as as_raw cs cs_raw note type],
  attack_rows
)

write_tsv(File.join(out_dir, 'creature_messages.tsv'), %w[creature_id kind key text], message_rows)

write_tsv(File.join(out_dir, 'creature_info.tsv'), %w[creature_id section text], info_rows)

write_tsv(File.join(out_dir, 'creature_lists.tsv'), %w[creature_id list name note], list_rows)

write_tsv(
  File.join(out_dir, 'creature_abilities.tsv'),
  %w[creature_id id name type target typical_duration_s dispellable effects notes],
  ability_rows
)

warn "creatures: #{files.length}"
warn "empty message and tip strings skipped: #{empty_strings}"
unless unparsed.empty?
  warn "Strengths kept verbatim because they are malformed in the source (#{unparsed.length}):"
  unparsed.each { |id, category, name, value| warn "  #{id}: #{category} #{name.inspect} #{value.inspect}" }
end

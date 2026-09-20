#!/usr/bin/env ruby
# frozen_string_literal: true

#
# Extract Lich's combat definition grammar into the TSV that `cena-model` ships.
#
# Run from the workspace root, against the author's LIVE Lich (the reference/
# snapshot is identical for these files -- `inventory/11` §0):
#
#   ruby crates/cena-model/tools/extract_combat_defs.rb \
#        C:/Gemstone/lich-5/lib/gemstone/combat/defs \
#        crates/cena-model/data
#
# A COMMITTED tool, for the reason `extract_crit_tables.rb` records: the defs
# grow with every hunt log the author mines (`COMBAT_DEFS_ONBOARDING.md` lists
# a 5,083-template backlog), and a script nobody kept means the next
# regeneration is hand work.
#
# ## Why Ruby
#
# The defs are Ruby regex literals with `#{MK_PRE}` / `#{MK_POST}` interpolated
# into them. `Regexp#source` gives the pattern exactly as Ruby compiled it,
# with the interpolation resolved, and `Regexp#options` gives its flags. A
# regex-over-regexes parse of the source files would have to reimplement
# Ruby's lexer to get the same answer.
#
# ## THE ONE TRANSFORMATION: markup tolerance is stripped
#
# Lich's patterns run against the RAW XML feed, so 178 of them carry `MK_PRE`
# (`(?:<pushBold/>)?(?:<a [^>]*>)?`) and `MK_POST` (`(?:</a>)?(?:<popBold/>)?`)
# at every place a link or bold tag may fall inside the text
# (`pattern_gate.rb:25-32`). Cena's classifiers run against a `ChunkLine`'s
# PLAIN text -- the parser already removed the markup and kept every link on
# the line as a typed `Run` -- so those optional groups can never match and
# are removed here. That is the only edit made to any pattern, and it is a
# pure deletion of groups that were optional to begin with.
#
# ## What is NOT transformed, and is flagged instead
#
# A few patterns read the `exist` id OUT OF THE TAG -- `<a exist="(?<id>\d+)"`
# -- which plain text cannot satisfy. They are written with `markup=1` in the
# `extra` column and left verbatim: the Rust side counts and skips them, and
# each is hand-ported to a text capture plus a link-position lookup, which is
# what "no second parser" means in practice. MEASURED at extraction time and
# printed; see the summary this tool prints.
#
# Deliberately NOT extracted:
#   - `messages.rb`: its defs carry a Ruby lambda per row that builds the
#     event payload, and it is a separate hook the combat FSM never reads
#     (`combat/messages.rb`). A later pass, if a consumer wants it.
#   - `supplements.rb`: player-supplied YAML defs. `plan/12`'s settled
#     decisions rule out user-authored automation; whether pattern supplements
#     count is an author question, not one this tool should answer by
#     including them.
#
# ## Order is load-bearing and is written down
#
# Every family is first-match-wins (`Attacks::ALL_ATTACKS` is assembled in a
# deliberate order: priority defs before generic swings, second-person before
# third-person -- `attacks.rb:762-777`). The `order` column is each pattern's
# position in its family's lookup, and the Rust loader sorts by it.

require 'fileutils'

src_dir = ARGV[0] or abort 'usage: extract_combat_defs.rb <defs-dir> <out-dir>'
out_dir = ARGV[1] or abort 'usage: extract_combat_defs.rb <defs-dir> <out-dir>'
defs_dir = File.expand_path(src_dir)

# ---------------------------------------------------------------------------
# The harness
# ---------------------------------------------------------------------------

# The def files `require_relative 'supplements'`, which pulls in YAML and
# Lich's UserDefs. Marking the file as already loaded makes that require a
# no-op, and the stub below answers the ten calls the defs make on it with
# "no supplements" -- which is byte-for-byte the shipped table
# (`supplements.rb:14-16`: "With no file present every reader returns an empty
# frozen array, so the assembled tables are byte-identical").
$LOADED_FEATURES << File.join(defs_dir, 'supplements.rb')

module Lich
  module Gemstone
    module Combat
      module Definitions
        module Supplements
          def self.attacks(_slot = nil) = []
          def self.flares = []
          def self.outcomes = []
          def self.statuses = []
          def self.messages(_name) = []
          def self.message_families = []
          def self.assembled!(_kind) = nil
          def self.report_match_timeout(_pattern) = nil
          def self.report_union_failure(_label, _error) = nil
          def self.reload_defs! = nil
        end
      end
    end
  end
end

require File.join(defs_dir, 'pattern_gate')
%w[assaults attacks damage flares outcomes sequences spell_losses statuses ucs].each do |name|
  require File.join(defs_dir, name)
end

D = Lich::Gemstone::Combat::Definitions

# ---------------------------------------------------------------------------
# Rows
# ---------------------------------------------------------------------------

MK_PRE  = D::MK_PRE
MK_POST = D::MK_POST
# `(?:<pushBold/>)?` also appears on its own in 14 patterns, written before
# `MK_PRE` existed; it is the first half of MK_PRE and just as optional.
BARE_BOLD = ['(?:<pushBold/>)?', '(?:<pushBold\/>)?'].freeze

# A tag that survived stripping. `(?<name>` is a named group, not a tag.
RESIDUAL_MARKUP = /(?<!\(\?)<[A-Za-z\/]/

rows = []
markup_rows = []
duplicate_names = []

# Strip the optional markup-tolerance groups. String#gsub with a String
# pattern is a literal replace, so nothing in MK_PRE is read as regex.
def plainify(source)
  s = source.dup
  s = s.gsub(MK_PRE, '').gsub(MK_POST, '')
  BARE_BOLD.each { |b| s = s.gsub(b, '') }
  s
end

def flags_of(regex)
  (regex.options & Regexp::IGNORECASE).zero? ? '' : 'i'
end

# Write one pattern row. `extra` is a `k=v;k=v` bag whose keys differ per
# family; `order` is the pattern's position in its family's first-match-wins
# lookup.
add_row = lambda do |family, name, role, order, regex, extra = {}|
  source = regex.source
  plain = plainify(source)
  if plain.match?(RESIDUAL_MARKUP)
    extra = extra.merge(markup: 1)
    markup_rows << [family, name, source]
    plain = source # verbatim: the Rust side hand-ports these
  end
  names = source.scan(/\(\?<([A-Za-z_]\w*)>/).flatten
  dups = names.tally.select { |_, c| c > 1 }.keys
  duplicate_names << [family, name, dups] unless dups.empty?
  raise "tab or newline in a pattern: #{family}/#{name}" if plain.match?(/[\t\n]/)

  rows << [family, name.to_s, role.to_s, order, flags_of(regex),
           extra.map { |k, v| "#{k}=#{v}" }.join(';'), plain]
end

# --- attacks ---------------------------------------------------------------
# Walked group by group rather than through ALL_ATTACKS, so each row records
# WHICH group it came from (`group=`): `attackerless_line?` is defined over
# the ENVIRONMENTAL_ATTACKS group (`attacks.rb:615`), and the assembly order
# is the priority rule (`attacks.rb:762-777`). The concatenation is asserted
# equal to ALL_ATTACKS so a Lich reorder cannot pass through silently.
ATTACK_GROUPS = %w[PRIORITY_ATTACKS BASIC_ATTACKS SPELL_ATTACKS WIKI_SPELL_ATTACKS
                   MANEUVER_ATTACKS WEAPON_ATTACKS SHIELD_ATTACKS COMPANION_ATTACKS
                   ENVIRONMENTAL_ATTACKS THIRD_PERSON_SPELL_ATTACKS THIRD_PERSON_ATTACKS].freeze
assembled = ATTACK_GROUPS.flat_map { |g| D::Attacks.const_get(g) }
raise 'ATTACK_GROUPS no longer matches ALL_ATTACKS -- Lich reordered the assembly' unless
  assembled == D::Attacks::ALL_ATTACKS

order = 0
ATTACK_GROUPS.each do |group|
  tag = group.sub(/_ATTACKS\z/, '').downcase
  D::Attacks.const_get(group).each do |d|
    d.patterns.compact.each { |rx| add_row.call('attack', d.name, '', order += 1, rx, group: tag) }
  end
end
# Classification lists: not patterns, but facts about attack names the FSM
# reads. Kept in the same file so the data is in one place.
# Numbered continuously across the three roles, so `order` is 1..=n for this
# family as for every other: a name list has no first-match order, but the
# loader's invariant is simpler if no family is the exception.
class_order = 0
{ environmental: D::Attacks::ENVIRONMENTAL,
  self_inflicted: D::Attacks::SELF_INFLICTED,
  room_targeted: D::Attacks::ROOM_TARGETED }.each do |role, names|
  names.each { |n| rows << ['attack_class', n.to_s, role.to_s, class_order += 1, '', '', ''] }
end
D::Attacks::COUP_KILL_PATTERNS.each_with_index do |(rx, loc), i|
  add_row.call('coup_kill', loc, '', i + 1, rx)
end
D::Attacks::AMBUSH_PREFIXES.each_with_index do |rx, i|
  add_row.call('ambush_prefix', D::Attacks::AMBUSH_KINDS[i], '', i + 1, rx)
end
D::Attacks::REACTION_PREFIXES.each_with_index do |(rx, kind), i|
  add_row.call('reaction_prefix', kind, '', i + 1, rx)
end
D::Attacks::REDIRECT_PREFIXES.each_with_index do |rx, i|
  add_row.call('redirect_prefix', 'guardian', '', i + 1, rx)
end

# --- damage ----------------------------------------------------------------
order = 0
{ basic: D::Damage::BASIC_DAMAGE, spell: D::Damage::SPELL_DAMAGE,
  environmental: D::Damage::ENVIRONMENTAL_DAMAGE, fallback: D::Damage::FALLBACK_DAMAGE }.each do |name, list|
  list.each { |rx| add_row.call('damage', name, '', order += 1, rx) }
end

# --- resolutions and outcomes ---------------------------------------------
order = 0
D::Resolutions::RESOLUTION_DEFS.each do |d|
  d.patterns.each { |rx| add_row.call('resolution', d.type, '', order += 1, rx) }
end
D::Resolutions::CRIT_RIDER_PATTERNS.each_with_index do |rx, i|
  add_row.call('crit_rider', 'knockdown', '', i + 1, rx)
end
order = 0
D::Outcomes::OUTCOME_DEFS.each do |d|
  d.patterns.each { |rx| add_row.call('outcome', d.type, '', order += 1, rx) }
end

# --- flares ----------------------------------------------------------------
order = 0
D::Flares::FLARE_DEFS.each do |d|
  extra = { damaging: d.damaging ? 1 : 0, aoe: d.aoe ? 1 : 0, spawns: d.spawns ? 1 : 0 }
  d.patterns.each { |rx| add_row.call('flare', d.name, '', order += 1, rx, extra) }
end
add_row.call('flare_weapon_link', 'weapon', '', 1, D::Flares::WEAPON_LINK)

# --- statuses --------------------------------------------------------------
order = 0
D::Statuses::STATUS_EFFECTS.each do |d|
  d.add_patterns.compact.each { |rx| add_row.call('status', d.name, 'add', order += 1, rx) }
end
D::Statuses::STATUS_EFFECTS.each do |d|
  d.remove_patterns.compact.each { |rx| add_row.call('status', d.name, 'remove', order += 1, rx) }
end

# --- spell losses ----------------------------------------------------------
order = 0
D::SpellLosses::SPELL_LOSSES.each do |d|
  d.patterns.each do |rx|
    # One def has `spell: nil` -- the game's generic "appears somehow
    # different" wear-off, which Lich deliberately pins to no number
    # (`spell_losses.rb:107`). Its name is `unknown` rather than an empty
    # column, so the loader keeps the row and the Rust side reads `None`.
    add_row.call('spell_loss', d.spell || 'unknown', '', order += 1, rx, spell_name: d.spell_name)
  end
end

# --- brackets --------------------------------------------------------------
{ 'assault' => D::Assaults::ASSAULT_DEFS, 'sequence' => D::Sequences::SEQUENCE_DEFS }.each do |family, defs|
  order = 0
  defs.each do |d|
    d.start_patterns.each { |rx| add_row.call(family, d.name, 'start', order += 1, rx) }
  end
  defs.each do |d|
    d.end_patterns.each { |rx| add_row.call(family, d.name, 'end', order += 1, rx) }
  end
end

# --- UCS -------------------------------------------------------------------
{ position: D::UCS::POSITION_PATTERN, position_inbound: D::UCS::POSITION_INBOUND_PATTERN,
  tierup: D::UCS::TIERUP_PATTERN, smite_applied: D::UCS::SMITE_APPLIED_PATTERN,
  smite_held: D::UCS::SMITE_HELD_PATTERN, smite_removed: D::UCS::SMITE_REMOVED_PATTERN }
  .each_with_index { |(name, rx), i| add_row.call('ucs', name, '', i + 1, rx) }
D::UCS::POSITION_TIERS.each_with_index do |(word, tier), i|
  rows << ['ucs_tier', word, '', i + 1, '', "tier=#{tier}", '']
end

# ---------------------------------------------------------------------------
# Output
# ---------------------------------------------------------------------------

FileUtils.mkdir_p(out_dir)

# Three files, split by what part of an exchange the family describes:
# what STARTS an attack, what RESOLVES it, and what rides on or follows it.
# Each is a real seam a reader would open on its own, and each stays under
# the workspace's 800-line file cap without a fifth exception -- one file
# would be 955 lines, and `caps.baseline` says the exception count should
# travel down, not up.
FILES = {
  'combat_attacks.tsv' => %w[attack attack_class coup_kill ambush_prefix reaction_prefix redirect_prefix],
  'combat_results.tsv' => %w[damage resolution crit_rider outcome],
  'combat_effects.tsv' => %w[flare flare_weapon_link status spell_loss assault sequence ucs ucs_tier]
}.freeze

placed = FILES.values.flatten
unplaced = rows.map(&:first).uniq - placed
raise "families with no output file: #{unplaced.inspect}" unless unplaced.empty?

FILES.each do |filename, families|
  path = File.join(out_dir, filename)
  subset = rows.select { |r| families.include?(r.first) }
  File.open(path, 'w') do |fh|
    fh.puts(%w[family name role order flags extra pattern].join("\t"))
    subset.each { |r| fh.puts(r.join("\t")) }
  end
  warn "#{filename}: #{subset.length} rows"
end

counts = rows.group_by(&:first).transform_values(&:length)
warn "total: #{rows.length} rows"
counts.sort_by { |_, c| -c }.each { |f, c| warn format('  %-18s %4d', f, c) }
warn "patterns kept verbatim with markup=1 (hand-ported in Rust): #{markup_rows.length}"
markup_rows.each { |f, n, s| warn "  #{f}/#{n}: #{s[0, 100]}" }
unless duplicate_names.empty?
  warn 'DUPLICATE NAMED GROUPS (Rust regex rejects these; rename before compiling):'
  duplicate_names.each { |f, n, d| warn "  #{f}/#{n}: #{d.join(',')}" }
end

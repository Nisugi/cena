#!/usr/bin/env ruby
# frozen_string_literal: true

#
# Extract Lich's 21 critical-hit tables into the TSV that `cena-model` ships.
#
# Run from the workspace root:
#
#   ruby crates/cena-model/tools/extract_crit_tables.rb \
#        reference/lich-5/lib/gemstone/critranks \
#        crates/cena-model/data/crit_tables.tsv
#
# This is a COMMITTED tool, not a throwaway. `plan/13` section 4a names the crit
# tables as a port target and `plan/13:125` says they "ship as data files"; the
# data therefore gets re-cut whenever Lich updates its tables, and a script
# nobody kept means the next regeneration is hand work.
#
# It needs no Lich runtime. Each `*critical_table.rb` file does nothing but
# assign into `CritRanks.table[:<type>]`, so a five-line stub of that module is
# the entire harness -- which is also why `reference/` being gitignored costs
# nothing: the Rust side consumes the checked-in TSV, never the .rb files, and
# `cargo test` never shells out to Ruby.
#
# ## The four data facts this tool normalises, each deliberately
#
# Measured over all 2,394 entries (see `crit_tables.rs`'s module docs for the
# commands). These are NOT silent fixes -- each one is asserted by the parity
# test, so a future Lich update that removes or multiplies them goes red.
#
# 1. **Three spellings of one key.** `:type` is written "non-corporeal" and
#    "ucs-jab" (hyphens) under hash keys `:non_corporeal` / `:ucs_jab`;
#    `:location` is written "left arm" and "Neck" (space, capital) under keys
#    `:left_arm` / `:neck`; a secondary wound's location is "right leg" where
#    the primary location key is `:right_leg`. Lich's own `clean_key`
#    (reference/lich-5/lib/gemstone/critranks.rb:63-68) is the normaliser --
#    strip, downcase, `[ -]` -> `_` -- and this tool applies exactly that rule
#    rather than inventing a second one.
#
# 2. **`:wound_ranks` (plural), once.** One secondary wound hash spells the key
#    plural. Read both spellings; the parity test asserts exactly one entry
#    needs it.
#
# 3. **`:limb_favored => "trie"`, once** (ucs_jab/right_arm/9): a String where
#    every other entry has a bool. Read as `true`.
#
# 4. **Missing fields, three entries.** generic/unspecified/0 and
#    ucs_kick/left_eye/7 omit dazed/limb_favored/roundtime/silenced/slowed;
#    non_corporeal/neck/6 omits `:location` entirely and takes it from its hash
#    key. Absent booleans read as false, absent roundtime as 0.
#
# Both `:type` and `:location` are dropped from the output: the tool ASSERTS
# each equals its own hash key after cleaning (it does, for all 2,394), so
# carrying both is the duplication `plan/05` section -1 forbids. The key is the
# truth.
#

module Lich
  module Gemstone
    # The whole harness: the table files only ever assign into this hash.
    module CritRanks
      @critical_table = {}
      def self.table
        @critical_table
      end
    end
  end
end
CritRanks = Lich::Gemstone::CritRanks

# Lich's own rule, reference/lich-5/lib/gemstone/critranks.rb:63-68.
def clean_key(key)
  return key.to_s.downcase if key.is_a?(Symbol)

  key.to_s.strip.downcase.gsub(/[ -]/, '_')
end

# `nil` reads as false: three entries omit their boolean fields entirely.
# `"trie"` is the one String, a typo for true in ucs_jab/right_arm/9.
def boolean(value, where, field)
  case value
  when true then '1'
  when false, nil then '0'
  when 'trie' then '1'
  else raise "#{where}: :#{field} is #{value.inspect}, not a boolean"
  end
end

COLUMNS = %w[
  type location rank damage position fatal stunned amputated crippled sleeping
  dazed limb_favored roundtime silenced slowed wound_rank secondary_location
  secondary_wound_rank pattern
].freeze

def extract(source_dir)
  files = Dir.glob(File.join(source_dir, '*critical_table.rb')).sort
  raise "no *critical_table.rb under #{source_dir}" if files.empty?

  files.each { |file| load file }

  rows = []
  CritRanks.table.each do |type_key, by_location|
    by_location.each do |location_key, by_rank|
      next if by_rank.nil?

      by_rank.each do |rank, record|
        next if record.nil? || record[:regex].nil?

        rows << row(type_key, location_key, rank, record)
      end
    end
  end
  [files, rows]
end

def row(type_key, location_key, rank, record)
  type = clean_key(type_key)
  location = clean_key(location_key)
  where = "#{type}/#{location}/#{rank}"

  # The record's own :type/:location must agree with its hash key once cleaned.
  # If they ever diverge the key is no longer the truth and dropping the fields
  # would lose data, so this is an assertion rather than a preference.
  if record[:type] && clean_key(record[:type]) != type
    raise "#{where}: :type #{record[:type].inspect} disagrees with its key"
  end
  if record[:location] && clean_key(record[:location]) != location
    raise "#{where}: :location #{record[:location].inspect} disagrees with its key"
  end

  secondary = record[:secondary_wound]
  if secondary
    secondary_location = clean_key(secondary[:location])
    # `:wound_ranks` (plural) is spelled that way in exactly one entry.
    secondary_rank = (secondary[:wound_rank] || secondary[:wound_ranks]).to_s
    raise "#{where}: secondary wound with no rank" if secondary_rank.empty?
  else
    secondary_location = ''
    secondary_rank = ''
  end

  pattern = record[:regex].source
  raise "#{where}: regex contains a tab or newline" if pattern =~ /[\t\n\r]/
  unless record[:regex].options.zero?
    raise "#{where}: regex has flags #{record[:regex].options}, which the TSV cannot carry"
  end

  [
    type, location, rank.to_s, record[:damage].to_s,
    record[:position] ? clean_key(record[:position]) : '',
    boolean(record[:fatal], where, :fatal), record[:stunned].to_s,
    boolean(record[:amputated], where, :amputated),
    boolean(record[:crippled], where, :crippled),
    boolean(record[:sleeping], where, :sleeping),
    boolean(record[:dazed], where, :dazed),
    boolean(record[:limb_favored], where, :limb_favored),
    (record[:roundtime] || 0).to_s,
    boolean(record[:silenced], where, :silenced),
    boolean(record[:slowed], where, :slowed),
    record[:wound_rank].to_s, secondary_location, secondary_rank, pattern
  ]
end

source_dir, output = ARGV
if source_dir.nil? || output.nil?
  abort "usage: #{$PROGRAM_NAME} <critranks-dir> <output.tsv>"
end

files, rows = extract(source_dir)

# Sorted by (type, location, rank), which is the order the Rust side binary
# searches and the order the golden digest is taken in. A stable order is what
# makes a regeneration a reviewable diff instead of a reshuffle.
#
# `location` sorts by LOCATION_ORDER, not alphabetically. That order is the
# declaration order of `Location` in crates/cena-model/src/crit/types.rs, which
# is what `#[derive(Ord)]` gives that enum and therefore what
# `CritTables::get`'s binary search requires. Sorting alphabetically here would
# still round-trip -- the two orderings hold the same 2,394 rows -- but the
# shipped file would be in a different order from the table built out of it,
# so a reader diffing the TSV against the in-memory table would see 2,578 lines
# of noise. VERIFIED: the two orderings were confirmed to differ only in order
# (`diff <(sort raw) <(sort canonical)` printed nothing).
LOCATION_ORDER = %w[
  head neck left_eye right_eye chest abdomen back left_arm right_arm
  left_hand right_hand left_leg right_leg nerves unspecified
].freeze

rows.each do |row|
  next if LOCATION_ORDER.include?(row[1])

  raise "#{row[0]}/#{row[1]}: location is not in LOCATION_ORDER; add it there "         'and to Location in crates/cena-model/src/crit/types.rs, in the same order'
end

rows.sort_by! { |r| [r[0], LOCATION_ORDER.index(r[1]), r[2].to_i] }

keys = rows.map { |r| r[0, 3] }
raise 'duplicate (type, location, rank)' unless keys.size == keys.uniq.size

# Binary mode with explicit "\n": on Windows, text mode would write "\r\n" and
# the checked-in file would differ by platform.
File.open(output, 'wb') do |file|
  file.write("#{COLUMNS.join("\t")}\n")
  rows.each { |r| file.write("#{r.join("\t")}\n") }
end

warn "#{files.size} table files -> #{rows.size} entries -> #{output}"

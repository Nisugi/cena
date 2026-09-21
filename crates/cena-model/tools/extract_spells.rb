#!/usr/bin/env ruby
# frozen_string_literal: true

#
# Extract Lich's spell table into the TSV that `cena-model` ships.
#
# Run from the workspace root:
#
#   ruby crates/cena-model/tools/extract_spells.rb \
#        C:/Gemstone/lich-5/data/effect-list.xml \
#        crates/cena-model/data/spells.tsv
#
# A COMMITTED tool, for the reason `extract_crit_tables.rb` records: the data
# gets re-cut whenever Lich updates it, and a script nobody kept means the next
# regeneration is hand work.
#
# ## What this is, and why it is a port target at all
#
# `lib/common/spell.rb` is 954 lines and it is **not** the spell table -- it is
# the READER for one. The table is `data/effect-list.xml`: 515 spells, 628
# messages, shipped with Lich and updated with it. That makes it the same shape
# as the crit tables, the bestiary and the armament aliases, and `plan/13`:125
# says that shape ships as a data file.
#
# ## The durations are Ruby, and 40 of them are real code
#
# This is the one place the port cannot be faithful, so it is measured rather
# than glossed. Of 338 `<duration>` bodies:
#
#   178  a plain number
#   120  arithmetic over circle ranks, level, or Spell[n].known?
#    40  REAL RUBY
#
# The 40 include five that `reget` the scrollback and re-parse a CS/TD line to
# work out how long a spell landed for:
#
#   if (history = reget) and (history = history.reverse) and
#      (line = history.find { |l| l =~ /^The air thickens and begins to swirl/ })
#      and history[history.index(line)+2] =~ /CS: ... == \+([0-9]+)/
#   then ($1.to_i - 100)/60.0 else 0.25 end
#
# That is a scripting language embedded in a data file. Cena has ruled out an
# embedded scripting language by settled decision (`CLAUDE.md`), and evaluating
# it would be exactly that.
#
# So: **the 298 that are data become data, and the 40 keep their Ruby verbatim
# in the `duration_raw` column and read as `Unknown`.** Per section 5.2, absent
# is not zero -- a consumer asking for one of those 40 gets `None` and can say
# it does not know, rather than being handed a fabricated 0.25.
#
# The split is emitted in the header comment of the TSV so a regeneration that
# changes these counts is visible in the diff.

require 'rexml/document'

SOURCE = ARGV[0] or abort "usage: #{$PROGRAM_NAME} <effect-list.xml> <out.tsv>"
DEST   = ARGV[1] or abort "usage: #{$PROGRAM_NAME} <effect-list.xml> <out.tsv>"

# A duration body that is just a number.
PLAIN = /\A-?\d+(\.\d+)?\z/

# A body built only from numbers, arithmetic, and the three lookups the model
# can answer: a spell circle's ranks, a stat, a skill, and `Spell[n].known?`.
# Anything else -- `reget`, `Spellsong.timeleft`, a block, a regex -- is code.
SIMPLE = %r{\A[\d\s.+\-*/()]*
            (?:(?:Spell\[\d+\]\.known\?|Spells\.\w+|Stats\.\w+|Skills\.\w+|\d+(?:\.\d+)?)
               [\s?:+\-*/().]*)*\z}x

# Tabs and newlines would break the TSV; a message pattern legitimately
# contains neither, so this is a guard rather than a transformation.
def clean(text)
  return '' if text.nil?

  text.to_s.gsub(/[\t\r\n]+/, ' ').strip
end

# Separators for the packed columns.
#
# **Not `|` and not `~`.** The duration bodies are Ruby, and four of them
# contain a literal `|` inside a block parameter:
#
#   history.find { |l| l =~ /^The air thickens/ }
#
# so joining with `|` split them mid-expression and the Rust reader saw
# fragments, classifying five as `unknown` that are not. It produced a
# green-looking table and was caught only because a parity test counted the
# kinds. `~` is no better: it is a field separator here and appears 919 times
# in the data.
#
# ASCII US (0x1f) and RS (0x1e) exist for exactly this, and MEASURED over the
# generated file both appear zero times in any value.
UNIT = "\x1f" # between the parts of one packed value
REC  = "\x1e" # between packed values in one column

def duration_kind(body)
  return 'fixed'   if body =~ PLAIN
  return 'derived' if body =~ SIMPLE

  'unknown'
end

doc = REXML::Document.new(File.read(SOURCE))
spells = doc.elements.to_a('//spell')
abort "no <spell> elements in #{SOURCE}" if spells.empty?

counts = Hash.new(0)
rows = spells.map do |spell|
  durations = spell.elements.to_a('duration').map do |d|
    body = clean(d.text)
    kind = duration_kind(body)
    counts[kind] += 1
    # cast-type is absent on most: a spell with one duration does not say
    # which form it is, and Lich defaults that to :self (`spell.rb`).
    [d.attributes['cast-type'] || 'self', kind, body].join(UNIT)
  end

  costs = spell.elements.to_a('cost').to_h { |c| [c.attributes['type'], clean(c.text)] }
  bonuses = spell.elements.to_a('bonus').map { |b| "#{b.attributes['type']}#{UNIT}#{clean(b.text)}" }
  messages = spell.elements.to_a('message').to_h { |m| [m.attributes['type'], clean(m.text)] }
  cooldowns = spell.elements.to_a('cooldown').to_h { |c| [c.attributes['type'], clean(c.text)] }

  [
    spell.attributes['number'],
    clean(spell.attributes['name']),
    spell.attributes['type'],
    spell.attributes['availability'],
    costs['mana'], costs['spirit'], costs['stamina'], costs['renew'],
    durations.join(REC),
    bonuses.join(REC),
    messages['start'],
    messages['end'],
    messages['target-start'],
    cooldowns['group'],
    cooldowns['target'],
  ].map { |v| v.nil? ? '' : v.to_s }
end

HEADER = %w[
  number name type availability
  mana_cost spirit_cost stamina_cost renew_cost
  durations bonuses
  msg_start msg_end msg_target_start
  cooldown_group cooldown_target
].freeze

File.open(DEST, 'w') do |out|
  out.puts "# Generated by tools/extract_spells.rb from Lich's data/effect-list.xml."
  out.puts '# Do not hand-edit: re-run the tool.'
  out.puts "# spells\t#{rows.length}"
  # The duration split, in the file, so a regeneration that changes it shows up
  # in the diff rather than being absorbed silently.
  out.puts "# durations\tfixed=#{counts['fixed']}\tderived=#{counts['derived']}\tunknown=#{counts['unknown']}"
  out.puts HEADER.join("\t")
  rows.each { |r| out.puts r.join("\t") }
end

warn "wrote #{rows.length} spells to #{DEST}"
warn "durations: fixed=#{counts['fixed']} derived=#{counts['derived']} unknown=#{counts['unknown']}"

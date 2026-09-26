#!/usr/bin/env ruby
# frozen_string_literal: true

#
# Extract `flare_patterns.rb`, the flare table the old Lich repository's
# scripts share, into the TSV that `cena-model` ships beside Lich's own defs.
#
# Run from the workspace root:
#
#   ruby crates/cena-model/tools/extract_flare_mirror.rb \
#        reference/lich_repo_mirror/lib/flare_patterns.rb \
#        crates/cena-model/data
#
# It reads `combat_effects.tsv` from the data directory (the shipped flares,
# `extract_combat_defs.rb`'s output) and writes `combat_flare_mirror.tsv`.
#
# ## What the file is
#
# `lib/flare_patterns.rb` in `reference/lich_repo_mirror` (changelog 1.0.0
# 2024-09-07 to 1.7.3 2026-06-21): three hashes of message patterns,
# `NODMGFLARE_PATTERNS`, `DMGFLARE_PATTERNS` and `ATTACK_PATTERNS`, loaded by
# `flarewindow.lic` and `flaretracker_2.lic`. It is the flare messaging Lich's
# shipped `combat/defs/flares.rb` does not have: custom and festival flare
# texts, purified metals, lore-flare repeats, weapon and armor scripts,
# Covert Arts poisons, gemstone properties, ensorcell (`inventory/12` §2,
# pile 2's Special ask 3).
#
# ## The rows, as the two scripts compile them
#
# Both scripts build ONE table, `NODMGFLARE_PATTERNS.merge(DMGFLARE_PATTERNS)`
# (`flarewindow.lic:734`, `flaretracker_2.lic:682`), take the first key whose
# pattern matches the plain-text line, and treat a key found in the
# non-damaging hash as non-damaging before asking whether it is damaging
# (`flarewindow.lic:1145-1150`). This tool writes exactly that table, in that
# order, with `damaging` decided the same way. `ATTACK_PATTERNS` is not a
# flare (`flarewindow.lic:1122` skips it too) and is not written.
#
# Two keys are in both hashes, `Infusion_GS` and `Trueshot_GS`. The merge
# keeps the damaging hash's value, `place holder`, at the non-damaging key's
# position, so the scripts never run the non-damaging `Infusion_GS` pattern.
# Neither does this table.
#
# A `place holder` pattern is a reserved name with no message yet. It is
# counted and skipped: as a regex it would only match the words "place holder".
# So are the keys in NOT_PORTED below: lines of a proc that a shipped flare
# already counts, where reading them too would count the proc twice.
#
# ## No edit is made to any pattern
#
# The scripts read Lich's plain-text stream (`get`, no downstream XML), which
# is what a `ChunkLine`'s text is here, so no markup group needs stripping.
# The source goes in verbatim. The patterns have no captures: the flare's
# creature is the line's bolded link, which the state machine reads as it does
# for every flare (`parse/flares.rs`, `target_info`).
#
# ## Names: a shipped flare keeps its name and its flags
#
# This is Lich's own rule for flare patterns added after the shipped defs
# (`defs/supplements.rb:778-798`, and `flares.rb`'s FLARE_LOOKUP comment): *"a
# name that reuses a shipped flare inherits the shipped flags"*, and *"a new
# name defaults every omitted flag to false"*. So:
#
# - a key that IS a shipped flare -- the same name lowercased (`Somnis`), or
#   an entry in ALIASES below -- is written under the shipped name with the
#   shipped `damaging`, `aoe` and `spawns`. A custom acid-flare text then
#   counts as an acid flare in `;combat`, which is what it is;
# - any other key is written lowercased (`Pure_Drakar` -> `pure_drakar`),
#   `damaging` as the scripts decide it, `aoe=0`, `spawns=0`.
#
# Every alias is checked, not trusted: some alternative of the key's pattern
# must share 12 consecutive literal characters with a shipped pattern of
# that name, or the tool stops. The alias names the flare that message
# already belongs to in Lich's own table.

require 'fileutils'

src = ARGV[0] or abort 'usage: extract_flare_mirror.rb <flare_patterns.rb> <data-dir>'
data_dir = ARGV[1] or abort 'usage: extract_flare_mirror.rb <flare_patterns.rb> <data-dir>'

load File.expand_path(src)

# Keys that name a shipped flare under another name. The comment is the
# shared text the check below finds.
ALIASES = {
  # non-damaging hash
  WThorns640_Poison: :wall_of_thorns,          # "One of the vines surrounding you lashes out at"
  SigilStaff_DoubleCast: :sigil_cast,          # "twining into an echo of your last spell"
  Sigil_of_Binding: :sigil_bane,               # "A bolt of energy leaps from your"
  NightshroudCloak_Hide: :shadow_shroud,       # "The shadows swirl around in an attempt to conceal you"
  NTouch625_ArcaneReflex: :arcane_reflex,      # "Vital energy infuses you, hastening your arcane reflexes"
  NTouch625_PhysicalProwess: :physical_prowess, # "The vitality of nature bestows you with a burst of strength"
  Dramatic_Drapery_Wondorous: :filament_corona, # "Branching filaments of power snap outward from your"
  Breeze612_Tailwind: :tailwind,               # "A favorable tailwind springs up behind you"
  Breeze612_Offensive: :breeze,                # "is buffeted by a burst of wind and pushed back"
  Acuity_Flare: :acuity,                       # "glows intensely with a verdant light"
  ChameleonShroud_GS: :chameleon_shroud,       # "A tenebrous shroud stitches itself into existence"
  HuntersAfterimage_GS: :hunters_afterimage,   # "appears in your ready hand, coalescing to replace"
  MirrorImage_GS: :mirror_image,               # "a mirror image of you shimmers into view at your side"
  # damaging hash
  Acid_Flare: :acid,                           # " a spray of acid"
  Air_Flare: :air,                             # " unleashes a blast of air"
  Air_LoreFlare: :air_flourish,                # " in a suffocating cyclone"
  Cold_Flare: :cold,                           # " intensely with a cold blue light"
  Cold_GEF: :cold_gef,                         # "A vortex of razor-sharp ice gust"
  Ice_GEF: :cold_gef,                          # "A vortex of razor-sharp ice gust"
  Disintegration_Flare: :disintegration,       # " a shimmering beam of disintegration"
  Dispel_Disruption: :dispel,                  # " brightly for a moment, consuming the magical energies around"
  Dispel_FluxCrit: :dispel_flux,               # "fluxes chaotically"
  Disruption_Flare: :disruption,               # " a quivering wave of disruption"
  Earth_LoreFlare: :earth_flourish,            # "Chunks of earth violently orbit"
  Earth_GEF: :earth_gef,                       # "A violent explosion of frenetic energy rumbles from"
  Energy_Weapon: :energy,                      # " energy emits from the tip of your"
  Fire_Flare: :fire,                           # " with a burst of flame"
  Fire_LoreFlare: :fire_flourish,              # " scorching everything in its wake"
  Fire_GEF: :fire_gef,                         # "Burning orbs of pure flame burst from"
  Grapple_Flare: :grapple,                     # " a twisted tendril of force"
  GuidingLight_2ndFlare: :guiding_light,       # " with a burst of plasma energy"
  HolyFire: :holy_fire,                        # " bursts alight with leaping tongues of holy fire"
  HolyWater: :holy_water,                      # " forth a shower of pure water"
  Impact_Flare: :impact,                       # " a blast of vibrating energy"
  Lightning_GEF: :lightning_gef,               # "A vicious torrent of crackling lightning surges from"
  Lightning_Flare: :lightning,                 # " a searing bolt of lightning"
  LowSteel: :psychic_assault,                  # " unleashes a blast of psychic energy"
  MindWrack_Flare: :psychic_assault,           # " unleashes a blast of psychic energy"
  Magma_Flare: :magma,                         # " a glob of molten magma"
  Mechanical_Flare: :spring_rod,               # " a small spring-loaded"
  Mirthbrand_GlitterFlare: :glittering_motes,  # "A winking spray of glittering motes erupts from your"
  Necromancy_LoreFlare: :necromancy_flourish,  # "A sickly green aura radiates from"
  Plasma_Flare: :plasma,                       # " with a burst of plasma energy"
  Pure_Adamantine: :unyielding_force,          # " with unyielding force!"
  Religion_LoreFlare: :religion_flourish,      # "Divine flames kindle around"
  SanguineSacrifice_Overflow: :bloodstone,     # "Sanguine brilliance strikes"
  SigilStaff_Dispel: :sigil_dispel,            # " lash out from your "
  SonicWeapon_1stFlare: :sonic_weapon,         # " unleashes a blast of sonic energy at"
  SonicWeapon_2ndFlare: :sonic_weapon,         # "With a loud snap, a blast of energy bursts from your"
  Spikes: :shield_spike,                       # "A spike on your"
  Steam_Flare: :steam,                         # " with a plume of steam"
  Summoning_LoreFlare: :summoning_flourish,    # "A radiant mist surrounds"
  Telepathy_LoreFlare: :telepathy_flourish,    # "Rippling and half-seen, strands of psychic power unravel from"
  Unbalance_Flare: :unbalance,                 # " unleashes an invisible burst of force"
  Vacuum_Flare: :vacuum,                       # " seems to fold inward upon itself drawing everything"
  Valence_SliceofShientyr: :valence,           # "A coil of spectral"
  Water_Flare: :water,                         # " a blast of water"
  WildfireOil: :alchemical_fire,               # "A swirl of alchemical fire"
  BS_Reckoning: :reckoning,                    # "Falling like a hammer from the sky, a golden light strikes the"
  Blessing_LoreFlare: :blessings_flourish,     # "A spiritual resonance warms your core"
  BloodBoil_GS: :boil_blood,                   # "A fiery aura spirals from"
  CA_ArachnesBite: :weapon_poison,             # " poison does its work"
  CA_FoolsDeathwort: :weapon_poison,
  CA_OphidiansKiss: :weapon_poison,
  CA_RavagersRevenge: :weapon_poison,
  CA_ShatterlimbPoison: :weapon_poison
}.freeze

# Keys NOT written, because the line is part of a proc Lich's shipped flares
# already count: read here as well, one proc is two flares in `;combat`.
# Each was found by the table reading a line of Lich's own replay fixtures
# (`crates/cena-model/tests/fixtures/combat/`); the four ensorcell benefits
# not seen there are INFERRED from the one that is.
NOT_PORTED = {
  Animalistic_FuryFlares: "the PREFIX of the shipped spirit_animal flare (flares.rb:325-326); " \
                          'fire.txt and hamstring.txt, the spirit_animal line next',
  Ensorcell_AS_CS: 'the benefit line after the shipped ensorcell line (** Necrotic energy ' \
                   'from your X overflows into you! **) in attack, bolt, relentless_assault, subdue',
  Ensorcell_Health: 'the same, INFERRED',
  Ensorcell_Mana: 'the same, INFERRED',
  Ensorcell_Spirit: 'the same, INFERRED',
  Ensorcell_Stamina: 'the same, INFERRED'
}.freeze

PLACEHOLDER = /\Aplace ?holder/

# --- the shipped flares: name -> [extra, [pattern text]] --------------------
shipped = {}
File.foreach(File.join(data_dir, 'combat_effects.tsv')).drop(1).each do |line|
  cols = line.chomp.split("\t", 7)
  next unless cols[0] == 'flare'

  entry = (shipped[cols[1]] ||= [cols[5], []])
  raise "flare/#{cols[1]}: two flag sets in the shipped table" unless entry[0] == cols[5]

  entry[1] << cols[6]
end

# The pattern with its regex syntax removed: what text it asks for, runs
# joined by NUL. Only the syntax this file and the shipped flares use.
def literal_runs(pattern)
  s = pattern.gsub(/\(\?<\w+>/, "\0").gsub(/\(\?:/, "\0")
  s = s.gsub(/\(s\)\?/, '').gsub(/s\?(?=[ !])/, '')
  s = s.gsub(/\[[^\]]*\][+*]?\??/, "\0").gsub(/\.[+*]\??/, "\0").gsub(/ \*\?/, "\0")
  s = s.gsub(/\\s\+/, ' ').gsub(/\\(.)/, '\1')
  s.gsub(/[()|^$]/, "\0").split("\0").map(&:strip).reject(&:empty?)
end

SHARED = 12

def shares_text?(mirror_source, shipped_patterns)
  runs = literal_runs(mirror_source).select { |r| r.length >= SHARED }
  texts = shipped_patterns.map { |p| literal_runs(p).join("\0") }
  runs.any? do |r|
    (0..(r.length - SHARED)).any? { |i| texts.any? { |t| t.include?(r[i, SHARED]) } }
  end
end

# --- the rows ------------------------------------------------------------------
merged = NODMGFLARE_PATTERNS.merge(DMGFLARE_PATTERNS)
rows = []
skipped = []
order = 0
merged.each do |key, rx|
  source = rx.source
  if source.match?(PLACEHOLDER)
    skipped << key
    next
  end
  next if NOT_PORTED.key?(key)

  raise "#{key}: flags #{rx.options} -- only plain patterns are expected" unless rx.options.zero?
  raise "#{key}: tab or newline in the pattern" if source.match?(/[\t\n]/)

  own = key.to_s.downcase
  name = ALIASES.fetch(key, own).to_s
  if (ship = shipped[name])
    if ALIASES.key?(key) && !shares_text?(source, ship[1])
      raise "#{key} -> #{name}: no alternative shares 12 characters with a shipped #{name} pattern"
    end
    # the three flags only: the shipped energy row also says `markup=1`,
    # which is about ITS pattern, not this one
    extra = ship[0].split(';').grep(/\A(?:damaging|aoe|spawns)=/).join(';')
  elsif ALIASES.key?(key)
    raise "#{key} -> #{name}: no shipped flare of that name"
  else
    damaging = NODMGFLARE_PATTERNS.key?(key) ? 0 : 1
    extra = "damaging=#{damaging};aoe=0;spawns=0"
  end
  rows << ['flare_mirror', name, '', order += 1, '', "#{extra};key=#{key}", source]
end

unused = (ALIASES.keys + NOT_PORTED.keys) - merged.keys
raise "aliases or exclusions for keys the file does not have: #{unused.inspect}" unless unused.empty?

out = File.join(data_dir, 'combat_flare_mirror.tsv')
FileUtils.mkdir_p(data_dir)
File.open(out, 'w') do |fh|
  fh.puts(%w[family name role order flags extra pattern].join("\t"))
  rows.each { |r| fh.puts(r.join("\t")) }
end

alternatives = rows.sum { |r| r[6].split(/(?<!\\)\|/).length }
reused = rows.count { |r| shipped.key?(r[1]) }
warn "#{File.basename(out)}: #{rows.length} rows, #{alternatives} message alternatives"
warn "  under a shipped flare's name: #{reused}; new names: #{rows.length - reused}"
warn "  damaging: #{rows.count { |r| r[5].start_with?('damaging=1') }}"
warn "  placeholders skipped: #{skipped.length} (#{skipped.join(', ')})"
warn "  not ported, part of a shipped flare's proc: #{NOT_PORTED.length} (#{NOT_PORTED.keys.join(', ')})"

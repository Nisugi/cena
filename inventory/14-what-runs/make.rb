# frozen_string_literal: true

# The list of what runs (plan/46 section 11 step 5), made from the
# checker's answers (bridges/ruby/hydra/check.rb):
#
#   ruby bridges/ruby/hydra/check.rb reference/scripts/scripts > A.json
#   ruby bridges/ruby/hydra/check.rb reference/lich_repo_mirror/lib > B.json
#   ruby inventory/14-what-runs/make.rb elanthia-online=A.json lich-repo=B.json
#
# writes one TSV a collection beside this file (a row a script: its verdict,
# what Hydra has to answer for it, and what is its own) and prints the
# tables inventory/14-what-runs.md shows.

require 'json'

# A finding, as a gap: one name for everything that stops scripts for one
# reason, so scripts can be counted by what they need.
def gap(finding)
  what = finding['what']
  case finding['kind']
  when 'markup' then 'the game\'s markup'
  when 'windows' then 'Lich\'s windows (Gtk)'
  when 'differs' then what.include?('pattern') ? 'a hook matching markup' : what
  else
    return own(finding) unless finding['hydra']
    return what.sub(/\ARoom\./, 'Map.') if what.match?(/\A(?:Spell|Room|Map)[#.]|\AXMLData\./)
    return 'Lich::Util' if what.start_with?('Lich::Util')

    what.sub(/\A(?:Lich::)?(?:Common::|Gemstone::|Games::Gemstone::)?/, '').split(/::|\./).first
  end
end

# What a script does itself: a name defined nowhere, a method Ruby 4.0
# dropped, a library not installed, a file Ruby cannot read.
def own(finding)
  case finding['why']
  when /cannot read it/ then 'does not parse'
  when /no such library/ then 'a library not installed here'
  when /not defined in Ruby/ then "#{finding['what']} (Ruby 4.0)"
  else 'a name defined nowhere'
  end
end

def percent(part, whole) = format('%.1f%%', 100.0 * part / whole)

collections = ARGV.map do |arg|
  name, file = arg.split('=', 2)
  [name, JSON.parse(File.read(file))]
end

VERDICTS = %w[runs differs markup windows stops].freeze

puts '### Verdicts', ''
puts "| Verdict | #{collections.map { |n, _| "#{n} files | lines" }.join(' | ')} |"
puts "|---|#{'---:|---:|' * collections.size}"
VERDICTS.each do |verdict|
  cells = collections.map do |_, scripts|
    picked = scripts.select { |s| s['verdict'] == verdict }
    lines = scripts.sum { |s| s['lines'] }
    "#{picked.size} (#{percent(picked.size, scripts.size)}) | " \
      "#{picked.sum { |s| s['lines'] }} (#{percent(picked.sum { |s| s['lines'] }, lines)})"
  end
  puts "| #{verdict} | #{cells.join(' | ')} |"
end

collections.each do |name, scripts|
  File.open(File.join(__dir__, "#{name}.tsv"), 'w') do |out|
    out.puts %w[script lines verdict builtin hydra_answers its_own].join("\t")
    scripts.sort_by { |s| s['name'].downcase }.each do |s|
      hydras = s['findings'].select { |f| f['hydra'] }.map { |f| gap(f) }.uniq
      owns = s['findings'].reject { |f| f['hydra'] }.map { |f| gap(f) }.uniq
      out.puts [s['name'], s['lines'], s['verdict'], s['builtin'] ? 'yes' : '', hydras.join('; '),
                owns.join('; ')].join("\t")
    end
  end
end

# What each script needs of Hydra to run, when nothing of its own stops it.
needs = collections.to_h do |name, scripts|
  [name, scripts.filter_map do |s|
    next if s['verdict'] == 'runs' || s['verdict'] == 'differs'
    next if s['findings'].any? { |f| !f['hydra'] }

    s['findings'].reject { |f| f['kind'] == 'differs' }.map { |f| gap(f) }.uniq
  end]
end

puts '', '### What Hydra has to answer, in the order that runs the most scripts', ''
puts 'Greedy: at each step the gap whose answer lets the most scripts run, then the one most needed.', ''
puts "| # | Gap | #{collections.map { |n, _| "#{n}: needing it | running after" }.join(' | ')} |"
puts "|---:|---|#{'---:|---:|' * collections.size}"
answered = []
all = needs.values.flatten(1)
20.times do |step|
  left = all.flatten.uniq - answered
  break if left.empty?

  best = left.max_by do |g|
    [all.count { |n| (n - answered - [g]).empty? }, all.count { |n| n.include?(g) }]
  end
  answered << best
  cells = collections.map do |name, scripts|
    base = scripts.count { |s| %w[runs differs].include?(s['verdict']) }
    running = needs[name].count { |n| (n - answered).empty? }
    "#{needs[name].count { |n| n.include?(best) }} | #{base + running} (#{percent(base + running, scripts.size)})"
  end
  puts "| #{step + 1} | #{best} | #{cells.join(' | ')} |"
end

puts '', '### What stops scripts that is their own', ''
collections.each do |name, scripts|
  owns = Hash.new(0)
  scripts.each do |s|
    s['findings'].reject { |f| f['hydra'] }.map { |f| gap(f) }.uniq.each { |g| owns[g] += 1 }
  end
  total = scripts.count { |s| s['findings'].any? { |f| !f['hydra'] } }
  puts "#{name}: #{total} scripts. " + owns.sort_by { |g, n| [-n, g] }.first(8).map { |g, n| "#{g} #{n}" }.join(', ')
end

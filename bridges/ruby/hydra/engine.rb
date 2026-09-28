# frozen_string_literal: true

# Lich's engine and Hydra's edges, loaded as a runner runs them: what both
# the runner (runner.rb) and the checker (check.rb) stand on, so what the
# checker finds defined is what a script finds.
#
# Whoever requires this sets SCRIPT_DIR, DATA_DIR and $lich_char first.

# Lich's own check (lib/version.rb, REQUIRED_RUBY) opens a dialog on
# Windows and exits; say it here instead, where Hydra hears it.
if Gem::Version.new(RUBY_VERSION) < Gem::Version.new('4.0')
  warn "[hydra] Ruby #{RUBY_VERSION} is too old for Lich's engine: 4.0 or newer."
  exit 2
end

LIB_DIR = File.expand_path('../lich/lib', __dir__)
LICH_DIR = DATA_DIR
TEMP_DIR = File.join(DATA_DIR, 'temp')
$clean_lich_char = $lich_char
$lich_char_regex = Regexp.escape($lich_char)
$frontend = 'stormfront'
$SEND_CHARACTER = '>'
$cmd_prefix = ''

# The gems Lich's engine needs beside Ruby, as Lich's installer provides
# them: its stores are SQLite through Sequel.
require 'sqlite3'
require 'sequel'

# What Lich loads before any script (lich.rbw), which scripts use without
# loading it themselves: `OpenStruct`, `Time.parse`, `YAML`,
# `Terminal::Table`... Each is loaded the first time a script names it
# (Ruby's autoload): loading them all at start took a runner's start from
# 0.9 s to 1.6 s (plan/46 section 9). `time`, which gives Time methods
# rather than a name, is loaded now. One not installed fails the script
# that uses it, naming it, as under a Lich without it.
{
  'Base64' => 'base64', 'Digest' => 'digest', 'DRb' => 'drb/drb', 'Monitor' => 'monitor',
  'Net::HTTP' => 'net/http', 'OpenSSL' => 'openssl', 'OpenStruct' => 'ostruct', 'Resolv' => 'resolv',
  'REXML' => File.expand_path('rexml.rb', __dir__), 'StringIO' => 'stringio',
  'Terminal' => 'terminal-table', 'YAML' => 'yaml'
}.each do |name, library|
  *outer, last = name.split('::')
  scope = outer.inject(Object) do |within, part|
    within&.const_defined?(part, false) ? within.const_get(part) : nil
  end
  if scope
    scope.autoload(last, library) unless scope.const_defined?(last, false)
  else
    Object.autoload(outer.first, library)
  end
end
%w[socket time].each { |library| require library }

# Lich started headless (`--no-gtk`, lib/init.rb): Hydra's runner opens no
# windows, and a script that asks takes its way without them.
HAVE_GTK = false

# Lich's engine, unchanged (../lich, BSD 3-Clause: ../lich/LICENSE.txt):
# the script itself, the calls scripts make, the `;` command table, the
# classes a script reads its character through (filled by copy.rb), and
# the stores.
%w[
  version.rb
  constants.rb
  common/class_exts/nilclass.rb
  common/limitedarray.rb
  common/feature_flags.rb
  messaging.rb
  common/detachable_client_registry.rb
  common/markup.rb
  common/script.rb
  common/downstreamhook.rb
  common/upstreamhook.rb
  global_defs.rb
  common/gameobj.rb
  attributes/char.rb
  lich.rb
].each { |file| require File.join(LIB_DIR, file) }
include Lich::Common

# The stores (plan/46 section 5): Lich's own, writing `lich.db3` in
# DATA_DIR as Lich writes it in its data folder. Its tables are made first,
# as Lich makes them at start: the settings' adapter would otherwise make
# one without the key its saves need.
Dir.mkdir(TEMP_DIR) unless Dir.exist?(TEMP_DIR)
Lich.init_db
%w[
  common/settings.rb
  common/settings/charsettings.rb
  common/settings/gamesettings.rb
  common/vars.rb
  common/uservars.rb
].each { |file| require File.join(LIB_DIR, file) }

require_relative 'connection'
require_relative 'edge'
require_relative 'copy'
require_relative 'map'
require_relative 'spell'
require_relative 'builtins'
require_relative 'hooks'
require_relative 'listener'

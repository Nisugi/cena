# frozen_string_literal: true

# Hydra's Ruby script runner (plan/46, M7b): one character's Lich scripts,
# run on Lich's own engine, with the game, the player's screen and the
# player's `;` commands answered by Hydra over hydra-script/1
# (crates/cena-agent/SCRIPTS.md).
#
# Hydra starts it, one per character, with where to reach Hydra and who it
# is in the environment, and it runs until Hydra dismisses it or the
# character's session ends. Its standard error is Hydra's to keep.

# Lich's own check (lib/version.rb, REQUIRED_RUBY) opens a dialog on
# Windows and exits; say it here instead, where Hydra hears it.
if Gem::Version.new(RUBY_VERSION) < Gem::Version.new('4.0')
  warn "[hydra] Ruby #{RUBY_VERSION} is too old for Lich's engine: 4.0 or newer."
  exit 2
end

def hydra_setting(name)
  ENV.fetch(name) do
    warn "[hydra] #{name} is not set: Hydra starts the runner with it."
    exit 2
  end
end

LIB_DIR = File.expand_path('../lich/lib', __dir__)
SCRIPT_DIR = hydra_setting('HYDRA_SCRIPTS')
DATA_DIR = hydra_setting('HYDRA_DATA')
LICH_DIR = DATA_DIR
TEMP_DIR = File.join(DATA_DIR, 'temp')
$lich_char = hydra_setting('HYDRA_SYMBOL')
$clean_lich_char = $lich_char
$lich_char_regex = Regexp.escape($lich_char)
$frontend = 'stormfront'
$SEND_CHARACTER = '>'
$cmd_prefix = ''

# Lich's engine, unchanged (../lich, BSD 3-Clause: ../lich/LICENSE.txt):
# the script itself, the calls scripts make, the `;` command table, and the
# classes a script reads its character through, filled by copy.rb.
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
  global_defs.rb
  common/gameobj.rb
  attributes/char.rb
].each { |file| require File.join(LIB_DIR, file) }
include Lich::Common

require_relative 'connection'
require_relative 'edge'
require_relative 'copy'
require_relative 'listener'

copy = Hydra::Copy.new
XMLData = Hydra::Data.new(hydra_setting('HYDRA_GAME'), hydra_setting('HYDRA_CHARACTER'), copy)
$stdout = Hydra::Screen.new
Hydra.connection = Hydra::Connection.new(hydra_setting('HYDRA_URL'), hydra_setting('HYDRA_TOKEN'))

Hydra::Listener.new(Hydra.connection, copy).run
# Hydra dismissed the runner, or the session ended: its scripts end with it.
exit 0

# frozen_string_literal: true

# Hydra's Ruby script runner (plan/46, M7b): one character's Lich scripts,
# run on Lich's own engine, with the game, the player's screen and the
# player's `;` commands answered by Hydra over hydra-script/1
# (crates/cena-agent/SCRIPTS.md).
#
# Hydra starts it, one per character, with where to reach Hydra and who it
# is in the environment, and it runs until Hydra dismisses it or the
# character's session ends. Its standard error is Hydra's to keep.

def hydra_setting(name)
  ENV.fetch(name) do
    warn "[hydra] #{name} is not set: Hydra starts the runner with it."
    exit 2
  end
end

SCRIPT_DIR = hydra_setting('HYDRA_SCRIPTS')
DATA_DIR = hydra_setting('HYDRA_DATA')
$lich_char = hydra_setting('HYDRA_SYMBOL')

require_relative 'engine'

copy = Hydra::Copy.new
XMLData = Hydra::Data.new(hydra_setting('HYDRA_GAME'), hydra_setting('HYDRA_CHARACTER'), copy)
$stdout = Hydra::Screen.new
Hydra.connection = Hydra::Connection.new(hydra_setting('HYDRA_URL'), hydra_setting('HYDRA_TOKEN'))

Hydra::Listener.new(Hydra.connection, copy).run
# Hydra dismissed the runner, or the session ended: its scripts end with it.
exit 0

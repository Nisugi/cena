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
# Whether scripts may open windows, as Hydra says: then the gtk3 gem, when
# the player has it (engine.rb).
HYDRA_WINDOWS = ENV['HYDRA_WINDOWS'] == '1'

require_relative 'engine'

copy = Hydra::Copy.new
XMLData = Hydra::Data.new(hydra_setting('HYDRA_GAME'), hydra_setting('HYDRA_CHARACTER'), copy)
$stdout = Hydra::Screen.new
Hydra.connection = Hydra::Connection.new(hydra_setting('HYDRA_URL'), hydra_setting('HYDRA_TOKEN'))

# Listen until Hydra dismisses the runner or the session ends; its scripts
# end with it. With Gtk, the main thread is the window loop's, as Lich's is
# (lich.rbw), and the loop is closed as Lich closes it when listening ends.
listening = Thread.new do
  Hydra::Listener.new(Hydra.connection, copy).run
ensure
  Lich::Common.shutdown_gtk_before_exit if HAVE_GTK
end
if HAVE_GTK
  Thread.current.priority = -10
  Gtk.main
  Lich::Common.shutdown_gtk_before_exit(direct: true)
else
  listening.join
end
exit 0

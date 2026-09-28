# frozen_string_literal: true

# The game's own classes, loaded once `XMLData` is made, as Lich's game
# loader loads them once its parser knows the game (common/gameloader.rb):
# some read the character as they load (psms/cman.rb asks `Effects` what
# a maneuver costs). Whoever requires this has required engine.rb and made
# `XMLData`.
#
# The character's sheet (plan/46 section 11, step 7): Lich's own readers,
# unchanged, over an `Infomon` answered from the copy (infomon.rb), and
# named at the top level as Lich names them (lib/main/main.rb, `include
# Lich::Gemstone`). `Lich::Util`'s commands that wait on the game's markup
# are step 10's.

require_relative 'infomon'
include Lich::Gemstone

module Lich
  module Gemstone
    # Lich's own names beside the game's, as lib/games.rb:1307 has it: so
    # `Resources` (`Lich::Resources`) is named at the top level too.
    include Lich

    # Lich's base of `Wounds`, `Scars` and `Injured`, as lib/games.rb
    # (BSD 3-Clause, ../lich/LICENSE.txt) writes it; the runner does not
    # load that file, Lich's game connection. Before a read, the game's
    # injury window is set to the mode wanted, and the read waits up to
    # 7.5 seconds for the game to say it is.
    class CharacterStatus
      class << self
        def fix_injury_mode(mode = 'both') # Default mode 'both' handles wounds (precedence) then scars
          case mode
          when 'scar', 'scars'
            unless XMLData.injury_mode == 1
              Game._puts '_injury 1'
              150.times { sleep 0.05; break if XMLData.injury_mode == 1 }
            end
          when 'wound', 'wounds' # future proof leaving in place, but this will likely not be used
            unless XMLData.injury_mode == 0
              Game._puts '_injury 0'
              150.times { sleep 0.05; break if XMLData.injury_mode == 0 }
            end
          when 'both'
            unless XMLData.injury_mode == 2
              Game._puts '_injury 2'
              150.times { sleep 0.05; break if XMLData.injury_mode == 2 }
            end
          else
            raise ArgumentError, "Invalid mode: #{mode}. Use 'scar', 'wound', or 'both'."
          end
        end

        def method_missing(_method_name = nil)
          result = Lich::Messaging.mono(Lich::Messaging.msg_format("bold", "#{self.name.split('::').last}: Invalid area, try one of these: arms, limbs, torso, #{XMLData.injuries.keys.join(', ')}"))
          # the _respond method used in Lich::Messaging returns nil upon success
          return result
        end
      end
    end
  end
end
# Each is loaded the first time a script names it (Ruby's autoload), as the
# libraries Lich loads before any script are (engine.rb): loaded at start,
# they took some 100 ms of a runner's start (plan/46 section 9), half of it
# `ostruct`. `Lich::Util` is loaded now, for the one change below.
require File.join(LIB_DIR, 'util', 'util.rb')
{
  Lich::Gemstone => {
    'gemstone/effects.rb' => %i[Effects],
    'attributes/stats.rb' => %i[Stats],
    'attributes/spells.rb' => %i[Spells],
    'attributes/skills.rb' => %i[Skills],
    'gemstone/society.rb' => %i[Society Societies],
    'gemstone/experience.rb' => %i[Experience],
    'gemstone/psms.rb' => %i[PSMS Armor Ascension CMan Feat QStrike Shield Warcry Weapon],
    'gemstone/currency.rb' => %i[Currency],
    'gemstone/injured.rb' => %i[Injured],
    'gemstone/wounds.rb' => %i[Wounds],
    'gemstone/scars.rb' => %i[Scars],
    'gemstone/stance.rb' => %i[Stance]
  },
  Lich => { 'attributes/resources.rb' => %i[Resources] }
}.each do |scope, files|
  files.each do |file, names|
    names.each { |name| scope.autoload(name, File.join(LIB_DIR, file)) }
  end
end

# Lich::Util's commands (plan/46 section 11, step 10) run as Lich's own:
# the command sent, its lines read as the script reads the game's, and with
# `quiet` hidden from the player by a display hook (hooks.rb). Those that
# read the game's markup -- `usexml`, `issue_command`'s default, and
# `quiet_command_xml` -- are the markup's (section 6.2, step 11), and say so
# rather than wait out their timeout for lines that never come.
module Hydra
  module UtilMarkup
    def issue_command(*args, usexml: true, **options)
      if usexml
        raise NotImplementedError, "Lich::Util.issue_command reads the game's markup (usexml), which Hydra does not " \
                                   'give scripts yet (plan/46): pass usexml: false and match the text'
      end

      super
    end
  end
end
Lich::Util.singleton_class.prepend(Hydra::UtilMarkup)

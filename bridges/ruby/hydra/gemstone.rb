# frozen_string_literal: true

# The game's own classes, loaded once `XMLData` is made, as Lich's game
# loader loads them once its parser knows the game (common/gameloader.rb):
# some read the character as they load (psms/cman.rb asks `Effects` what
# a maneuver costs). Whoever requires this has required engine.rb and made
# `XMLData`.
#
# The character's sheet (plan/46 section 11, step 7): Lich's own readers,
# unchanged, over an `Infomon` answered from the copy (infomon.rb), in the
# order the game loader loads them, and named at the top level as Lich
# names them (lib/main/main.rb, `include Lich::Gemstone`). `Lich::Util`'s
# commands that wait on the game's markup are step 10's.

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
%w[
  util/util.rb
  gemstone/effects.rb
  attributes/resources.rb
  attributes/stats.rb
  attributes/spells.rb
  attributes/skills.rb
  gemstone/society.rb
  gemstone/experience.rb
  gemstone/psms.rb
  gemstone/currency.rb
  gemstone/injured.rb
  gemstone/wounds.rb
  gemstone/scars.rb
].each { |file| require File.join(LIB_DIR, file) }
%i[issue_command quiet_command quiet_command_xml silver_count].each do |name|
  Hydra.unanswered(Lich::Util.singleton_class, name, 'send the command and wait for its lines with dothistimeout')
end

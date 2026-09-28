# frozen_string_literal: true

# Lich's own path from a script's line to its reply, in one process, for
# plan/46 section 9 (crates/cena-agent/tests/measure.rs): Lich's engine as
# Hydra's runner loads it, a socket on this machine for the game that
# answers each line with one line, and a reader thread handing each line
# it reads to the scripts, as Lich's does (lib/games.rb) -- without Lich's
# XML parse, so a little quicker than Lich.
#
#   ruby lichpath.rb RUNNER_DIR OUT COUNT
#
# Its folders are made beside OUT, for whoever asked to remove.

require 'fileutils'
require 'json'
require 'socket'

runner_dir, out, count = ARGV
SCRIPT_DIR = DATA_DIR = File.join(File.dirname(out), 'lichpath')
FileUtils.mkdir_p(DATA_DIR)
$lich_char = ';'
require File.join(runner_dir, 'hydra', 'engine.rb')
XMLData = Hydra::Data.new('GS3', 'Bench', Hydra::Copy.new)

server = TCPServer.new('127.0.0.1', 0)
Thread.new do
  client = server.accept
  client.write("You see a quiet room.\r\n") while client.gets
end
GAME = TCPSocket.new('127.0.0.1', server.addr[1])
GAME.sync = true

# Lich's `Game._puts`: straight to the game's socket.
class Game
  def self._puts(line)
    GAME.write("#{line}\n")
  end
end

# Lich's reader thread: each line to the scripts.
Thread.new do
  while (line = GAME.gets)
    Script.new_downstream(line.chomp)
  end
end

code = <<~CODE
  silence_me
  times = []
  #{count.to_i}.times do
    started = Time.now.to_f
    put 'look'
    script.gets(5)
    times << Time.now.to_f - started
    sleep 0.02
  end
  File.write(#{out.inspect}, JSON.generate(times))
CODE
ExecScript.start(code, { quiet: true })
sleep 0.1 until File.exist?(out)
exit!(0)

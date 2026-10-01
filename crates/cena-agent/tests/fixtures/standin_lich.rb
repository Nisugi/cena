# frozen_string_literal: true

# A stand-in for Lich in pipe mode, for the relay's tests (tests/lich_relay.rs,
# plan/51): what the relay relies on, and nothing of Lich's engine.
#
# As Lich does (reference/lich-5/lib/main/main.rb:564-606 and 837-843), it
# reads a key and a version line from its standard input, connects to -g's
# host and port, and says both there; then the game's bytes it is handed go to
# its standard output. Closing its standard input stops it.
#
# What is typed on its standard input: a `;` line starts a script, which here
# puts the rest of the line after the script's name, as Lich's scripts put,
# with `<c>`, but for `;echo`, which shows it, as a script's `respond` does;
# anything else goes to the game as typed, after one alias, `gg` for
# `get gem`, as Lich's alias script would have it.
#
# What it shows, on its standard output, is the game's bytes but for a line
# saying `hide me`, as a squelch hides it.
#
# Once it has seen a prompt it acts as a script would, once: it puts `look`
# as Lich's scripts do, with `<c>`, and says what its frontend is, having set
# it to `unknown` as pipe mode does (main.rb:569).
#
# With STANDIN_STALL set it is a Lich whose script's hook never returns:
# once it has acted it reads nothing more, from its game or its standard
# input, and only being killed ends it.

require 'socket'

$frontend = 'unknown'
host, port = ARGV[ARGV.index('-g') + 1].split(':')
key = $stdin.gets
version = $stdin.gets
game = TCPSocket.new(host, Integer(port))
game.write(key, version)
$stdout.sync = true
stall = ENV.key?('STANDIN_STALL')

Thread.new do
  acted = false
  loop do
    chunk = game.readpartial(4096)
    $stdout.write(chunk.lines.reject { |line| line.include?('hide me') }.join)
    next if acted || !chunk.include?('<prompt')

    acted = true
    game.write("<c>look\n", "<c>frontend #{$frontend}\n")
    loop { sleep 60 } if stall
  end
rescue IOError, SystemCallError
  nil
end

# A sleep with no end would be Ruby's deadlock, and end it.
loop { sleep 60 } if stall
while (line = $stdin.gets)
  if line.start_with?(';echo ')
    $stdout.write("#{line.sub(/\A;echo /, '').chomp}\r\n")
  elsif line.start_with?(';')
    game.write("<c>#{line.sub(/\A;\S+\s*/, '')}")
  else
    game.write(line.chomp == 'gg' ? "get gem\n" : line)
  end
end
game.close

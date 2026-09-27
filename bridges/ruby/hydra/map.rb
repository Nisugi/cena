# frozen_string_literal: true

# Lich's `Room`, answered from Hydra's map (plan/46 section 4.3;
# crates/cena-agent/SCRIPTS.md, `room`): a runner carries no map of its own
# -- the map is most of what makes Lich heavy -- so each room is asked of
# Hydra once, by the map's own number, and kept.
#
# `Room.current` is the room Hydra names in the local copy (`map_room`),
# which it never guesses: where Lich's map would pick the first room that
# fits, this is nil, and a script sees it cannot tell.
#
# How an exit is crossed and what it costs, as Lich's `wayto` and `timeto`:
# a command is a String; a crossing Hydra ports as steps or a routine is a
# callable that asks Hydra's travel to walk it. A cost is seconds when it is
# a constant (or a Haste-shortened roundtime, at its full price); a cost
# Hydra answers only for a walker -- a gate, a ladder, a price table -- is a
# proc answering nil, as an exit Hydra cannot answer is impassable there.

module Hydra
  class Room
    # How long a crossing's walk may take before it is given up on.
    WALK = 30

    @rooms = {}
    @lock = Mutex.new

    class << self
      # The room the character is in, when Hydra's map names it.
      def current
        self[XMLData.map_room]
      end

      # A room by the map's number; nil when there is none or no map.
      def [](id)
        id = id.to_s
        return nil unless id =~ /\A\d+\z/

        @lock.synchronize do
          return @rooms[id] if @rooms.key?(id)
        end
        answer = Hydra.connection.call('room', { id: id.to_i })
        room = answer['room'] && new(answer['room'])
        @lock.synchronize { @rooms[id] = room }
      rescue Unanswered => e
        Lich.log "error: Room[#{id}]: #{e.message}"
        nil
      end
    end

    attr_reader :id, :uid, :title, :description, :paths, :location, :climate, :terrain, :tags,
                :wayto, :timeto

    def initialize(record)
      @id = record['id']
      @uid = Array(record['uid'])
      @title = Array(record['title'])
      @description = Array(record['description'])
      @paths = Array(record['paths'])
      @location = record['location']
      @climate = record['climate']
      @terrain = record['terrain']
      @tags = Array(record['tags'])
      @wayto = {}
      @timeto = {}
      Array(record['exits']).each do |exit|
        to = exit['to'].to_s
        @wayto[to] = exit['cmd'] || Crossing.new(exit['to'])
        @timeto[to] = Room.price(exit['cost'])
      end
    end

    # Seconds, or a proc answering them or nil: see the file's header.
    def self.price(cost)
      case cost
      when Numeric then cost.to_f
      when Hash
        return (cost['hasted'].to_f + cost['step'].to_f) if cost.key?('hasted')

        proc {}
      else proc {}
      end
    end

    def to_s
      "##{@id}: #{@title.first}"
    end

    def inspect = "#<Room #{self}>"
  end

  # An exit Hydra's travel crosses: `call` walks it, as Lich's StringProc
  # does, and answers whether the character arrived.
  class Crossing
    def initialize(to)
      @to = to
    end

    def call
      put "#{$lich_char}go2 #{@to}"
      deadline = Time.now + Room::WALK
      sleep 0.1 until XMLData.map_room == @to || Time.now > deadline
      XMLData.map_room == @to
    end

    def to_s = "#{$lich_char}go2 #{@to}"
  end
end

Room = Hydra::Room
Map = Hydra::Room

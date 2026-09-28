# frozen_string_literal: true

# Lich's `Room` and `Map`, answered from Hydra's map (plan/46 sections 4.3
# and 11, steps 2 and 8; crates/cena-agent/SCRIPTS.md, `room`, `route`,
# `seconds`, `rooms`, `find`, `tags`): a runner carries no map of its own
# -- the map is most of what makes Lich heavy -- so each room is asked of
# Hydra, by the map's own number, and kept.
#
# `Room.current` is the room Hydra names in the local copy (`map_room`),
# which it never guesses: where Lich's map would pick the first room that
# fits, this is nil, and a script sees it cannot tell.
#
# How an exit is crossed and what it costs, as Lich's `wayto` and `timeto`:
# a command is a String; a crossing Hydra ports as steps or a routine is a
# callable that has Hydra's travel walk it (builtins.rb, `go2`). A cost is seconds when it is
# a constant (or a Haste-shortened roundtime, at its full price); a cost
# Hydra answers only for a walker -- a gate, a ladder, a price table -- is a
# proc answering nil, as an exit Hydra cannot answer is impassable there.
#
# The shortest ways (`dijkstra`, `path_to`, the `find_nearest` family) and a
# path's time (`estimate_time`) are Hydra's to answer, priced for the
# character as travel prices its own walk; around them, Lich's own methods
# as lib/common/map/map_base.rb (BSD 3-Clause, ../lich/LICENSE.txt) writes
# them. `Map.list` is every room, asked a page at a time the first time a
# script asks and kept: each by its number, title, location, tags and the
# game's numbers, its description and exits asked when a script first reads
# them. The tags, titles and locations are shared: 36,838 rooms carry 477,451
# tags, most of them the same few thousand.

module Hydra
  class Room
    @rooms = {}
    @lock = Mutex.new
    @listing = Mutex.new
    @list = nil

    class << self
      attr_reader :lock

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
        @lock.synchronize { @rooms[id] ||= room }
      rescue Unanswered => e
        Lich.log "error: Room[#{id}]: #{e.message}"
        nil
      end

      # Every room of the map, as Lich's list: an Array by the map's
      # numbers, nil where the map has none; empty without a map.
      def list
        @listing.synchronize { @list ||= listed }
      end

      # Tag names any room carries.
      def tags = ask('tags', {}, 'tags')
      alias tag_names tags

      # The rooms carrying a tag, by number, in order.
      def rooms_by_tag(tag_name) = ask('find', { tag: tag_name.to_s }, 'ids')

      # The rooms the game numbers `uid`.
      def ids_from_uid(uid) = ask('find', { uid: uid.to_i }, 'ids')

      # What walking `array`'s rooms in order takes, in seconds: each step
      # as the character's walk prices it, 0.2 for one it cannot, as Lich.
      def estimate_time(array)
        raise Exception.exception('MapError'), 'Map.estimate_time was given something not an array!' unless array.is_a?(Array)

        Hydra.connection.call('seconds', { path: array.map(&:to_i) })['seconds'].to_f
      end

      # Lich's dispatcher (map_base.rb): the source by number, name or room.
      def dijkstra(source, destination = nil, static_only: false)
        room = source.is_a?(self) ? source : self[source]
        if room
          room.dijkstra(destination, static_only: static_only)
        else
          echo 'Map.dijkstra: error: invalid source room'
          nil
        end
      end

      # A room made from `rooms`' short record, kept, and completed when
      # read (`complete!`).
      def listed_room(record)
        id = record['id'].to_s
        @lock.synchronize { @rooms[id] ||= new(record, complete: false) }
      end

      private

      def listed
        list = []
        after = nil
        loop do
          answer = Hydra.connection.call('rooms', after ? { after: after } : {})
          break unless answer['map']

          answer['rooms'].each do |record|
            room = listed_room(record)
            list[room.id] = room
          end
          break unless (after = answer['next'])
        end
        list
      rescue Unanswered => e
        Lich.log "error: Map.list: #{e.message}"
        list
      end

      def ask(tool, arguments, field)
        Array(Hydra.connection.call(tool, arguments)[field])
      rescue Unanswered => e
        Lich.log "error: Map #{tool}: #{e.message}"
        []
      end
    end

    attr_reader :id, :uid, :title, :location, :tags

    def initialize(record, complete: true)
      @id = record['id']
      @uid = Array(record['uid'])
      @title = Array(record['title']).map { |title| -title }
      @location = record['location'] && -record['location']
      @tags = Array(record['tags']).map { |tag| -tag }
      @complete = false
      fill(record) if complete
    end

    # What only the whole record has, asked of Hydra the first time.
    %i[description paths climate terrain image image_coords wayto timeto].each do |name|
      define_method(name) do
        complete!
        instance_variable_get(:"@#{name}")
      end
    end

    # Lich's older names (map_base.rb).
    def desc = description
    def map_name = image
    def to_i = @id

    # Outdoors: its exits line says `Obvious paths:` (map_base.rb).
    def outside?
      return false if paths.nil? || paths.empty?

      paths.last =~ /^Obvious paths:/ ? true : false
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

    # The shortest ways out of this room, as Lich's: `[previous, seconds]`,
    # each keyed by room number, over the rooms the search settled. To one
    # room it stops there; to several, at the nearest (Lich's own stops
    # early only under 20 seconds, and otherwise searches the whole map);
    # to none, everywhere. `static_only` is Lich's way to skip what it
    # would evaluate; Hydra evaluates nothing of the script's, so it is the
    # same search. Nil when the search could not be made.
    def dijkstra(destination = nil, static_only: false)
      to = destination.nil? ? nil : Array(destination).map(&:to_i)
      answer = Hydra.connection.call('route', to ? { from: @id, to: to } : { from: @id })
      return nil unless answer['map']

      [answer['previous'].to_h { |room, from| [room.to_i, from] },
       answer['seconds'].to_h { |room, seconds| [room.to_i, seconds] }]
    rescue Unanswered => e
      echo "Map.dijkstra: error: #{e.message}"
      nil
    end
    alias dijkstra_hashes dijkstra

    # The rooms to pass through to `destination`, excluding this one and
    # including it; nil when there is no way (map_base.rb, `path_to`).
    def path_to(destination)
      destination = destination.to_i
      previous, = dijkstra(destination)
      return nil if previous.nil?
      return nil unless previous[destination]

      path = [destination]
      seen = { destination => true }
      until previous[path[-1]] == @id
        step = previous[path[-1]]
        return nil if step.nil? || seen[step]

        seen[step] = true
        path.push(step)
      end
      path.reverse
    end

    # The nearest room with a tag (map_base.rb, `find_nearest_by_tag`).
    def find_nearest_by_tag(tag_name)
      target_list = self.class.rooms_by_tag(tag_name)
      return @id if target_list.include?(@id)

      _, shortest_distances = dijkstra(target_list)
      return nil if shortest_distances.nil?

      target_list.delete_if { |room_num| shortest_distances[room_num].nil? }
      target_list.min_by { |room_num| shortest_distances[room_num] }
    end

    # Every room with a tag that can be reached, nearest first
    # (map_base.rb, `find_all_nearest_by_tag`).
    def find_all_nearest_by_tag(tag_name)
      target_list = self.class.rooms_by_tag(tag_name)
      _, shortest_distances = dijkstra
      return [] if shortest_distances.nil?

      target_list.delete_if { |room_num| shortest_distances[room_num].nil? }
      target_list.sort_by { |room_num| shortest_distances[room_num] }
    end

    # The nearest of `target_list` (map_base.rb, `find_nearest`).
    def find_nearest(target_list)
      target_list = target_list.collect(&:to_i)
      return @id if target_list.include?(@id)

      _, shortest_distances = dijkstra(target_list)
      return nil if shortest_distances.nil?

      target_list.select { |room_num| shortest_distances[room_num].is_a?(Numeric) }
                 .min_by { |room_num| shortest_distances[room_num] }
    end

    def to_s
      "##{@id}: #{@title.first}"
    end

    def inspect = "#<Room #{self}>"

    private

    # Ask Hydra for the whole record of a room made from `rooms`' short one.
    def complete!
      return if @complete

      record = Hydra.connection.call('room', { id: @id })['room'] || {}
      Room.lock.synchronize { fill(record) unless @complete }
    rescue Unanswered => e
      Lich.log "error: Room[#{@id}]: #{e.message}"
    end

    def fill(record)
      @description = Array(record['description'])
      @paths = Array(record['paths'])
      @climate = record['climate']
      @terrain = record['terrain']
      @image = record.dig('image', 'file')
      @image_coords = record.dig('image', 'rect')
      @wayto = {}
      @timeto = {}
      Array(record['exits']).each do |exit|
        to = exit['to'].to_s
        @wayto[to] = exit['cmd'] || Crossing.new(exit['to'])
        @timeto[to] = Room.price(exit['cost'])
      end
      @complete = true
    end
  end

  # An exit Hydra's travel crosses: `call` walks it, as Lich's StringProc
  # does, and answers whether the walk arrived.
  class Crossing
    def initialize(to)
      @to = to
    end

    def call = Builtins.perform("go2 #{@to}")

    def to_s = "#{$lich_char}go2 #{@to}"
  end
end

Room = Hydra::Room
Map = Hydra::Room

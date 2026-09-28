# frozen_string_literal: true

# Hydra's script checker (plan/46 section 1): which lines of a Lich script
# will not work under Hydra, and why, found without running it.
#
#   ruby check.rb FILE...        JSON: for each script its verdict, and each
#                                finding with its line
#   ruby check.rb --tsv FILE...  one row a script: name, lines, verdict, what
#
# A folder stands for the .lic scripts in it. The script is read by Ruby's
# own parser (Prism), and every name it uses is judged against the runner
# itself: this loads Lich's engine and Hydra's edges exactly as a runner
# does (engine.rb), so a name the checker finds defined is one the script
# finds defined, and nothing here lists what Hydra answers by hand. What is
# listed by hand is what the runner defines and does not answer as Lich
# does: the game's markup (plan/46 section 6.2), and each method marked
# not answered yet (`Hydra.unanswered`).
#
# Each finding is one of four kinds, and a script's verdict is its worst:
#
#   stops    the line raises under Hydra: a name not defined, a method not
#            answered, a library missing, or the script does not parse. The
#            script stops there, if it gets there.
#   markup   the line reads the game's markup, which Hydra does not give
#            scripts: it finds none, and what waits for it waits in vain.
#   windows  the line opens one of Lich's windows (Gtk): a window of its
#            own while Hydra lets the runner load the gtk3 gem and the
#            player has it (for now it does); where it cannot, the script
#            stops there, which is often only its settings window.
#   differs  it runs, and does not do what it did under Lich: a hook whose
#            pattern is markup, Lich's own state read as nil.
#
# and `runs` when nothing is found. A name the checker cannot see into -- a method of an unknown receiver, a
# constant from a gem the script loads -- is not judged, so `runs` means
# nothing was found, not that everything was proven.

require 'fileutils'
require 'json'
require 'prism'
require 'tmpdir'

SCRIPT_DIR = ENV.fetch('HYDRA_SCRIPTS', Dir.pwd)
# Where Lich's engine keeps its data as it loads: the runner's, as Hydra
# passes it, or a folder of the checker's own, gone when it is done.
DATA_DIR = ENV.fetch('HYDRA_DATA') do
  Dir.mktmpdir('hydra-check').tap { |dir| at_exit { FileUtils.rm_rf(dir) } }
end
$lich_char = ENV.fetch('HYDRA_SYMBOL', ';')

require_relative 'engine'

XMLData = Hydra::Data.new('GS3', 'Checker', Hydra::Copy.new)

module Hydra
  module Check
    # A thing found: where, how bad, what and why, and whether it is
    # Hydra's to answer (`hydra`) or the script's own: a name defined
    # nowhere, a method Ruby 4.0 dropped, a library not installed.
    Finding = Struct.new(:line, :kind, :what, :why, :hydra)

    # Lich's names the runner loads, whose missing methods are Lich's
    # engine's, not loaded or not answered: Hydra's to answer.
    LICH_NAMES = %w[GameObj Char Script Settings CharSettings GameSettings UserVars Vars DownstreamHook
                    UpstreamHook Win32].freeze

    # Lich's reads of the game's markup: defined in Lich's engine, never
    # answered under Hydra, which gives scripts the game's text and its
    # facts as data, not its markup (plan/46 section 6.2).
    MARKUP = 'the game\'s markup, which Hydra does not give scripts: they read its text, ' \
             'and its facts as data (plan/46 6.2)'
    # From worst to least: a verdict is the worst kind found.
    KINDS = %w[stops markup windows differs].freeze
    MARKUP_CALLS = %i[status_tags toggle_status want_downstream_xml want_downstream_xml=].freeze
    MARKUP_GLOBALS = /\A\$_(?:SERVER|CLIENT|DETACHABLE|LASTUPSTREAM)/

    # Lich's own state, which no script sees under Hydra.
    INTERNAL = 'Lich\'s own state, which Hydra\'s runner does not keep: nil'
    INTERNAL_GLOBALS = %w[
      $_IDLETIMESTAMP_ $safe_pause_lock $pause_all_lock $psinet $infomon_debug $creature_debug
      $setupfiles $fill_hands_actions $fill_left_hand_actions $fill_right_hand_actions
      $speech_highlight_start $speech_highlight_end $link_highlight_start $link_highlight_end
      $strip_xml_multiline $sftowiz_multiline
    ].freeze

    # Lich's windows: Lich loads the gtk3 gem for its scripts.
    WINDOWS = %w[Gtk Gdk GdkPixbuf GLib Pango Cairo].freeze
    WINDOWS_WHY = 'one of Lich\'s windows: opened through the gtk3 gem, which the runner loads when Hydra ' \
                  'lets it (for now) and the player has it; without it the script stops here'

    # Lich's classes a runner does not load yet (inventory/13 section 2.5).
    LICH_WHY = 'Lich\'s, not answered by Hydra yet (plan/46)'
    LICH_CLASSES = %w[
      Stats Skills Spells Effects Society Bounty Experience Wounds Scars Injured CMan Feat Armor Shield
      Weapon Warcry PSMS Ascension Enhancive Resources Currency Group Infomon Gift ReadyList StowList
      SpellRanks Spellsong CritRanks Stance Overwatch Armaments Mana Claim Disk Creature CreatureInstance
      Watchfor SessionVars DB_Store SK Util StringProc Preset Log
    ].freeze

    # Hydra's own answers to Lich's names: a missing method on one is Hydra's
    # to answer, not Ruby's or Lich's.
    HYDRAS = %w[XMLData Room Map Spell Game Frontend].freeze

    # Markup in a hook's pattern, `<c>` aside: what a Wrayth frontend sends
    # before a typed line, which Hydra gives the input hooks too.
    TAG = /(?<![?\\])<(?!c>)\/?[A-Za-z]/

    # Blocks run as another object: a bare name inside may be its method
    # (Sequel's table blocks among them: `primary_key :id`).
    OPAQUE_BLOCKS = %i[instance_eval instance_exec class_eval class_exec module_eval module_exec
                       define_method new define create_table create_table? create_table! alter_table
                       create_join_table].freeze

    # Modules a script mixes in whose names are the runner's own.
    MIXINS = %w[Lich::Common Comparable Enumerable Kernel Singleton].freeze

    class << self
      # What `path` comes to: its name, lines, verdict and findings.
      def file(path)
        source = File.binread(path).force_encoding(Encoding::UTF_8)
        source = source.encode(Encoding::UTF_8, Encoding::ISO_8859_1) unless source.valid_encoding?
        findings = findings(source, siblings(File.dirname(path)))
        name = File.basename(path, '.*')
        {
          'name' => name,
          # Hydra has it built in: its own runs when it is typed or started.
          'builtin' => Hydra::Builtins::COMMANDS.key?(name.downcase),
          'file' => path,
          'lines' => source.count("\n") + (source.end_with?("\n") ? 0 : 1),
          'verdict' => verdict(findings),
          'findings' => findings.sort_by(&:line).map(&:to_h).map { |h| h.transform_keys(&:to_s) }
        }
      end

      # The scripts beside one: a constant named after one of them (`LNet`,
      # lnet.lic; `Oleani`, oleani-lib.lic) is that script's, defined when it
      # runs in the same runner.
      def siblings(dir)
        @siblings ||= {}
        # Dir.glob reads a backslash as an escape: a Windows path is turned first.
        @siblings[dir] ||= Dir.glob(File.join(dir.tr('\\', '/'), '*.{lic,rb}'))
                              .map { |f| File.basename(f, '.*').downcase }
      end

      def findings(source, siblings = [])
        results = parsed(source)
        failed = results.find { |result| !result.errors.empty? }
        if failed
          error = failed.errors.first
          cut = results.size > 1 ? ', read as Lich reads it: cut at each line that is a label alone' : ''
          return [Finding.new(error.location.start_line, 'stops', 'does not parse',
                              "Ruby #{RUBY_VERSION} cannot read it#{cut}: #{error.message}", false)]
        end
        known = Known.new(siblings)
        results.each { |result| result.value.accept(known) }
        visitor = Visitor.new(known)
        results.each { |result| result.value.accept(visitor) }
        visitor.findings.uniq { |f| [f.line, f.what] }
      end

      # The script as Lich runs it (lib/common/script.rb, `@labels`): cut at
      # each label line, every part evaluated on its own in one binding, in
      # which `script` is the script. So each part is parsed on its own, with
      # the locals of the parts before it.
      def parsed(source)
        locals = [:script]
        sections(source).map do |line, code|
          result = Prism.parse(code, line: line, scopes: [locals.dup])
          locals |= result.value.locals if result.errors.empty?
          result
        end
      end

      def sections(source)
        sections = [[1, +'']]
        source.each_line.with_index(1) do |line, number|
          if line.chomp.match?(/^[\d_\w]+:$/)
            sections << [number + 1, +'']
          else
            sections.last[1] << line
          end
        end
        sections
      end

      def verdict(findings)
        KINDS.find { |kind| findings.any? { |f| f.kind == kind } } || 'runs'
      end
    end

    # What the script defines, and the libraries it loads: a first pass.
    class Known < Prism::Visitor
      attr_reader :methods, :constants, :findings, :guarded, :siblings
      # Names come from where this checker cannot see: a gem the script
      # loads, another script it starts, a module it mixes in.
      attr_accessor :unseen

      def initialize(siblings = [])
        super()
        @siblings = siblings
        @methods = Set.new
        @constants = Set.new
        @findings = []
        @guarded = Set.new
        @unseen = false
      end

      # What the script asks after before it uses it: `defined?(X.y)`.
      def visit_defined_node(node)
        guard(node.value)
        super
      end

      def visit_def_node(node)
        @methods << node.name
        super
      end

      def visit_class_node(node)
        @constants << node.constant_path.slice.split('::').last.to_sym
        super
      end

      def visit_module_node(node)
        @constants << node.constant_path.slice.split('::').last.to_sym
        super
      end

      def visit_constant_write_node(node)
        @constants << node.name
        super
      end

      def visit_constant_or_write_node(node)
        @constants << node.name
        super
      end

      def visit_constant_path_write_node(node)
        @constants << node.target.slice.split('::').last.to_sym
        super
      end

      def visit_call_node(node)
        case node.name
        when :attr_accessor, :attr_reader, :attr_writer, :define_method, :alias_method
          symbols(node).each do |name|
            @methods << name
            @methods << :"#{name}=" unless node.name == :attr_reader
          end
        when :require
          library(node) if node.receiver.nil?
        when :respond_to?
          symbols(node).each { |name| @guarded << name.to_s }
        when :start_script, :force_start_script, :load
          @unseen ||= starts_another?(node) if node.receiver.nil?
        when :run, :start
          @unseen ||= starts_another?(node) if node.receiver&.slice == 'Script'
        when :include, :extend
          @unseen ||= mixes_in_another?(node) if node.receiver.nil?
        end
        super
      end

      def visit_alias_method_node(node)
        @methods << node.new_name.unescaped.to_sym if node.new_name.respond_to?(:unescaped)
        super
      end

      private

      def guard(node)
        return unless node

        case node
        when Prism::ConstantReadNode, Prism::ConstantPathNode
          @guarded << node.slice.delete_prefix('::')
        when Prism::CallNode
          @guarded << node.name.to_s
        end
        node.compact_child_nodes.each { |child| guard(child) }
      end

      def mixes_in_another?(node)
        Array(node.arguments&.arguments).any? do |arg|
          next false unless arg.is_a?(Prism::ConstantReadNode) || arg.is_a?(Prism::ConstantPathNode)

          !MIXINS.include?(arg.slice.delete_prefix('::')) && !@constants.include?(arg.slice.split('::').last.to_sym)
        end
      end

      # A script started by name whose names may be what this one uses: not
      # one of Hydra's built-ins, which define none.
      def starts_another?(node)
        name = node.arguments&.arguments&.first
        !(name.is_a?(Prism::StringNode) && Hydra::Builtins::COMMANDS.key?(name.unescaped.downcase))
      end

      def symbols(node)
        Array(node.arguments&.arguments).filter_map do |arg|
          arg.unescaped.to_sym if arg.is_a?(Prism::SymbolNode) || arg.is_a?(Prism::StringNode)
        end
      end

      # A library the script loads: Ruby's own is loaded here too, so its
      # names are defined; a gem's is not loaded, and its names are not
      # judged; one that is nowhere stops the script.
      def library(node)
        arg = node.arguments&.arguments&.first
        return unless arg.is_a?(Prism::StringNode)

        name = arg.unescaped
        spec = Gem::Specification.find_by_path(name)
        if spec.nil? || spec.default_gem?
          require name
        else
          @unseen = true
        end
      rescue LoadError, StandardError, ScriptError
        @unseen = true
        @findings << Finding.new(node.location.start_line, 'stops', "require '#{name}'",
                                 'no such library here: a gem not installed, or a file Lich had', false)
      end
    end

    # The second pass: each name used, judged.
    class Visitor < Prism::Visitor
      attr_reader :findings

      def initialize(known)
        super()
        @known = known
        @findings = known.findings.dup
        @opaque = 0
        @rescued = 0
        @hook = nil
      end

      def visit_class_node(node)
        opaque = node.superclass && !defined_here?(node.superclass)
        inside(opaque) { super }
      end

      def visit_singleton_class_node(node)
        inside(true) { super }
      end

      # What `defined?` asks after is never used there.
      def visit_defined_node(_node); end

      # `x rescue y`: what fails in `x` does not stop the script. A
      # `begin ... rescue` does not count: most report the error and end.
      def visit_rescue_modifier_node(node)
        rescued(true) { visit(node.expression) }
        visit(node.rescue_expression)
      end


      # A default is evaluated only when the argument is left out.
      def visit_optional_parameter_node(node)
        inside(true) { super }
      end

      def visit_optional_keyword_parameter_node(node)
        inside(true) { super }
      end

      def visit_constant_read_node(node)
        constant(node, node.name.to_s)
        super
      end

      def visit_constant_path_node(node)
        path = node.slice.delete_prefix('::')
        if path.match?(/\A[A-Z]\w*(::[A-Z]\w*)*\z/)
          constant(node, path)
        else
          super
        end
      end

      def visit_global_variable_read_node(node)
        global(node, node.name.to_s)
        super
      end

      def visit_call_node(node)
        if hook_added?(node)
          visit(node.receiver)
          hook(node)
          return
        end
        node.receiver.nil? ? bare(node) : sent_to(node)
        if node.block && OPAQUE_BLOCKS.include?(node.name)
          visit(node.receiver) if node.receiver
          visit(node.arguments) if node.arguments
          inside(true) { visit(node.block) }
        else
          super
        end
      end

      def visit_regular_expression_node(node)
        markup_pattern(node, node.content) if @hook
        super
      end

      def visit_string_node(node)
        markup_pattern(node, node.unescaped) if @hook
        super
      end

      private

      def inside(opaque)
        @opaque += 1 if opaque
        yield
      ensure
        @opaque -= 1 if opaque
      end

      def rescued(on)
        @rescued += 1 if on
        yield
      ensure
        @rescued -= 1 if on
      end

      # Everything but a stop is Hydra's doing; a stop is Hydra's to answer
      # when `hydra` says so.
      def found(node, kind, what, why, hydra: kind != 'stops')
        return if kind == 'stops' && @rescued.positive?

        @findings << Finding.new(node.location.start_line, kind, what, why, hydra)
      end

      def defined_here?(node)
        node.respond_to?(:name) && @known.constants.include?(node.name)
      end

      # A constant: defined in the runner, by the script, or by a gem it
      # loads; otherwise the script stops there.
      def constant(node, path)
        return if own?(path) || guarded?(path) || resolve(path)

        head = path.split('::').first
        if WINDOWS.include?(head)
          found(node, 'windows', path, WINDOWS_WHY)
        elsif head == 'Lich' || lich_class?(path)
          found(node, 'stops', path, LICH_WHY, hydra: true)
        elsif !@known.unseen && !sibling?(head)
          found(node, 'stops', path, 'not defined under Hydra\'s runner, nor by this script')
        end
      end

      # A constant named after a script beside this one: that script's.
      def sibling?(name)
        name = name.downcase
        @known.siblings.any? { |stem| stem == name || stem.start_with?("#{name}-", "#{name}_") }
      end

      # A constant the script defines: its first or last name.
      def own?(path)
        names = path.split('::')
        @known.constants.include?(names.first.to_sym) || @known.constants.include?(names.last.to_sym)
      end

      # What the script asks after first (`defined?`, `respond_to?`): the
      # name, or any constant it is under.
      def guarded?(name)
        parts = name.split('::')
        parts.each_index.any? { |i| @known.guarded.include?(parts[0..i].join('::')) }
      end

      def resolve(path)
        path.split('::').inject(Object) do |scope, name|
          return nil unless scope.is_a?(Module) && scope.const_defined?(name)

          scope.const_get(name)
        end
      rescue NameError, LoadError
        nil
      end

      # A name Lich gives its API (inventory/13 section 2.5).
      def lich_class?(path)
        LICH_CLASSES.include?(path.split('::').last)
      end

      def global(node, name)
        if name.match?(MARKUP_GLOBALS)
          found(node, 'markup', name, MARKUP)
        elsif INTERNAL_GLOBALS.include?(name)
          found(node, 'differs', name, INTERNAL)
        end
      end

      # A bare call: the script's own, the runner's, or neither.
      def bare(node)
        name = node.name
        if MARKUP_CALLS.include?(name)
          found(node, 'markup', name.to_s.delete_suffix('='), MARKUP)
          return
        end
        return if @opaque.positive? || @known.unseen || @known.methods.include?(name) || guarded?(name.to_s)
        # `Date :created_at`, a capital name called: a library's own words.
        return if name.to_s.match?(/\A[A-Z]/)
        return if TOPLEVEL_BINDING.receiver.respond_to?(name, true)
        return if [Class, Module].any? { |m| m.private_method_defined?(name) || m.method_defined?(name) }

        found(node, 'stops', name.to_s, 'not defined under Hydra\'s runner, nor by this script')
      end

      # A call on something: judged when the receiver is a constant, or a
      # room or spell of Hydra's; not when it is anything else.
      def sent_to(node)
        name = node.name
        if MARKUP_CALLS.include?(name)
          found(node, 'markup', name.to_s.delete_suffix('='), MARKUP)
          return
        end
        return if guarded?(name.to_s)

        receiver = node.receiver
        case receiver
        when Prism::ConstantReadNode, Prism::ConstantPathNode
          path = receiver.slice.delete_prefix('::')
          return if own?(path) || guarded?(path)

          answered?(resolve(path), name) || unanswered(node, path, ".#{name}")
        when Prism::CallNode
          instance_of_hydras(node, receiver, name)
        end
      end

      def answered?(object, name)
        return true if object.nil? || object.respond_to?(name, true)

        object.method(:method_missing).owner != BasicObject
      rescue NameError
        true
      end

      def unanswered(node, path, call)
        head = path.split('::').first
        if HYDRAS.include?(path)
          found(node, 'stops', "#{path}#{call}", 'not answered by Hydra yet (plan/46)', hydra: true)
        elsif head == 'Lich' || LICH_NAMES.include?(path.split('::').last)
          found(node, 'stops', "#{path}#{call}", 'Lich\'s, not loaded or not answered by Hydra\'s runner yet (plan/46)',
                hydra: true)
        else
          found(node, 'stops', "#{path}#{call}", "not defined in Ruby #{RUBY_VERSION}")
        end
      end

      # `Room.current.path_to`, `Spell[401].cast`: a room's or a spell's own.
      def instance_of_hydras(node, receiver, name)
        owner = receiver.receiver
        return unless owner.is_a?(Prism::ConstantReadNode) && %i[Room Map Spell].include?(owner.name)
        return unless %i[current [] list].include?(receiver.name)
        return if receiver.name == :list

        klass = owner.name == :Spell ? Hydra::Spell : Hydra::Room
        label = "#{klass.name.split('::').last}##{name}"
        if Hydra.not_yet.key?(label)
          found(node, 'stops', label, "not answered by Hydra yet (plan/46): #{Hydra.not_yet[label]}", hydra: true)
        elsif !klass.method_defined?(name) && !klass.private_method_defined?(name)
          found(node, 'stops', label, 'not answered by Hydra yet (plan/46)', hydra: true)
        end
      end

      def hook_added?(node)
        node.name == :add && node.receiver &&
          %w[DownstreamHook UpstreamHook].include?(node.receiver.slice.split('::').last)
      end

      # A hook's patterns: markup never matches, since hooks see text.
      def hook(node)
        @hook = node.receiver.slice.split('::').last
        visit(node.arguments) if node.arguments
        visit(node.block) if node.block
      ensure
        @hook = nil
      end

      def markup_pattern(node, text)
        return unless text.match?(TAG)

        found(node, 'differs', "#{@hook} pattern #{text[0, 40]}",
              'hooks see the game\'s text, not its markup, so this never matches under Hydra (plan/46 6.1)')
      end
    end
  end
end

paths = ARGV.reject { |arg| arg.start_with?('--') }.flat_map do |arg|
  File.directory?(arg) ? Dir.glob(File.join(arg, '*.lic')).sort : [arg]
end
checked = paths.map { |path| Hydra::Check.file(path) }
if ARGV.include?('--tsv')
  checked.each do |script|
    what = script['findings'].map { |f| f['what'] }.uniq.join('; ')
    $stdout.puts [script['name'], script['lines'], script['verdict'], what].join("\t")
  end
else
  $stdout.puts JSON.generate(checked)
end

# frozen_string_literal: true

# REXML as Lich loads it (lich.rbw): the document and the stream listener,
# together, the first time a script names REXML (engine.rb). Scripts use
# REXML::StreamListener without loading it themselves.

require 'rexml/document'
require 'rexml/streamlistener'

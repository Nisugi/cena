# frozen_string_literal: true

# hydra-script/1's tools, over MCP's streamable HTTP with no session: each
# call a `tools/call` on its own (crates/cena-agent/SCRIPTS.md, Connecting).

require 'json'
require 'net/http'
require 'uri'

module Hydra
  # Hydra did not answer a call as asked: the listener unreachable, the
  # token refused, or the tool's own error.
  class Unanswered < StandardError; end

  # The runner's connection to Hydra.
  #
  # A fresh HTTP connection per call, so a `listen` waiting thirty seconds
  # holds nothing a script's `send` needs, and a call that failed is never
  # retried: a `send` retried after its bytes reached Hydra would reach the
  # game twice.
  class Connection
    PROTOCOL_VERSION = '2025-06-18'

    def initialize(url, token)
      @uri = URI(url)
      @token = token
      @ids = 0
      @lock = Mutex.new
    end

    # The tool's JSON answer. `wait` is how long, in seconds, the answer may
    # take to come.
    def call(tool, arguments = {}, wait: 15)
      id = @lock.synchronize { @ids += 1 }
      body = JSON.generate(jsonrpc: '2.0', id: id, method: 'tools/call',
                           params: { name: tool, arguments: arguments })
      response = post(body, wait)
      raise Unanswered, "#{tool}: HTTP #{response.code}" unless response.code == '200'

      reply = JSON.parse(response.body)
      raise Unanswered, "#{tool}: #{reply['error']['message']}" if reply['error']

      text = reply.dig('result', 'content', 0, 'text').to_s
      raise Unanswered, "#{tool}: #{text}" if reply.dig('result', 'isError')

      JSON.parse(text)
    rescue JSON::ParserError => e
      raise Unanswered, "#{tool}: #{e.message}"
    end

    private

    def post(body, wait)
      Net::HTTP.start(@uri.host, @uri.port, open_timeout: 5, read_timeout: wait) do |http|
        request = Net::HTTP::Post.new(@uri.request_uri, headers)
        request.body = body
        http.request(request)
      end
    rescue IOError, SystemCallError, Timeout::Error => e
      raise Unanswered, "#{e.class}: #{e.message}"
    end

    def headers
      {
        'Content-Type' => 'application/json',
        'Accept' => 'application/json, text/event-stream',
        'Authorization' => "Bearer #{@token}",
        'MCP-Protocol-Version' => PROTOCOL_VERSION
      }
    end
  end
end

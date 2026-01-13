# frozen_string_literal: true

require 'yaml'
require 'tempfile'
require 'fileutils'

module Threads
  # Custom exception classes
  class Error < StandardError; end
  class ThreadNotFound < Error; end
  class InvalidStatus < Error; end
  class WorkspaceError < Error; end
  class AmbiguousReference < Error; end
  class ParseError < Error; end
  class GitError < Error; end

  # Status constants
  TERMINAL_STATUSES = %w[resolved superseded deferred rejected].freeze
  ACTIVE_STATUSES = %w[idea planning active blocked paused].freeze
  ALL_STATUSES = (ACTIVE_STATUSES + TERMINAL_STATUSES).freeze

  # Validate status against allowed values
  def self.validate_status!(status)
    base = base_status(status)
    return if ALL_STATUSES.include?(base)

    raise InvalidStatus, "Invalid status: #{status}. Must be one of: #{ALL_STATUSES.join(', ')}"
  end

  # Check if status is terminal
  def self.terminal?(status)
    base = base_status(status)
    TERMINAL_STATUSES.include?(base)
  end

  # Check if status is valid
  def self.valid_status?(status)
    base = base_status(status)
    ALL_STATUSES.include?(base)
  end

  # Extract base status (strip reason suffix)
  def self.base_status(status)
    return '' if status.nil?

    idx = status.index(' (')
    idx ? status[0...idx] : status
  end

  # Thread represents a parsed thread file
  class Thread
    attr_accessor :path, :content, :frontmatter, :body_start

    def initialize(path)
      @path = path
      @content = ''
      @frontmatter = {}
      @body_start = 0
    end

    # Parse a thread file
    def self.parse(path)
      t = new(path)
      t.content = File.read(path)
      t.parse_frontmatter
      t
    end

    # Parse YAML frontmatter
    def parse_frontmatter
      unless @content.start_with?("---\n")
        raise ParseError, 'missing frontmatter delimiter'
      end

      # Find closing delimiter
      end_idx = @content.index("\n---", 4)
      raise ParseError, 'unclosed frontmatter' unless end_idx

      yaml_content = @content[4...end_idx]
      @body_start = end_idx + 5 # skip opening ---, yaml, closing ---, and newline

      begin
        @frontmatter = YAML.safe_load(yaml_content) || {}
      rescue Psych::SyntaxError => e
        raise ParseError, "invalid YAML frontmatter: #{e.message}"
      end

      # Extract ID from filename if not in frontmatter
      if id.nil? || id.to_s.empty?
        @frontmatter['id'] = Workspace.extract_id_from_path(@path)
      end
    end

    def id
      @frontmatter['id']
    end

    def name
      @frontmatter['name']
    end

    def status
      @frontmatter['status']
    end

    def desc
      @frontmatter['desc']
    end

    def base_status
      Threads.base_status(status)
    end

    def terminal?
      Threads.terminal?(status)
    end

    # Get body content (after frontmatter)
    def body
      return '' if @body_start >= @content.length

      @content[@body_start..]
    end

    # Set frontmatter field and rebuild content
    def set_field(field, value)
      @frontmatter[field] = value
      rebuild_content
    end

    # Rebuild content from frontmatter and body
    def rebuild_content
      body_content = body
      @content = "---\n"
      @content += YAML.dump(@frontmatter).sub(/^---\n?/, '')
      @content += "---\n"
      @body_start = @content.length
      @content += body_content if body_content && !body_content.empty?
    end

    # Write thread to disk atomically
    def write
      atomic_write(@path, @content)
    end

    # Relative path from workspace
    def rel_path(ws)
      @path.sub("#{ws}/", '')
    end

    private

    # Atomic file write using temp file and rename
    def atomic_write(path, content)
      dir = File.dirname(path)
      Tempfile.create(['thread', '.tmp'], dir) do |f|
        f.write(content)
        f.close
        FileUtils.mv(f.path, path)
      end
    rescue SystemCallError => e
      raise Threads::Error, "Failed to write #{path}: #{e.message}"
    end
  end
end

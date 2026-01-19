# frozen_string_literal: true

require 'fileutils'
require 'pathname'
require 'securerandom'

module Threads
  # Pre-compiled regexes for ID/name extraction (avoid recompilation per call)
  ID_PREFIX_RE = /^([0-9a-f]{6})-/.freeze
  NAME_EXTRACT_RE = /^[0-9a-f]{6}-(.+)$/.freeze
  ID_ONLY_RE = /^[0-9a-f]{6}$/.freeze
  # Options for finding threads with direction and boundary controls
  class FindOptions
    attr_accessor :down, :up

    def initialize
      @down = nil  # nil = no down search, Integer = depth (0=unlimited)
      @up = nil    # nil = no up search, Integer = depth (0=unlimited)
    end

    def has_down?
      !@down.nil?
    end

    def has_up?
      !@up.nil?
    end

    # Returns depth limit, -1 for unlimited
    def down_depth
      return 0 unless has_down?
      @down == 0 ? -1 : @down
    end

    # Returns depth limit, -1 for unlimited
    def up_depth
      return 0 unless has_up?
      @up == 0 ? -1 : @up
    end
  end

  # Workspace utilities for finding and navigating thread directories
  module Workspace
    class << self
      # Check if a path is contained within a directory (secure path containment)
      # This uses proper path canonicalization to prevent path traversal attacks.
      # Example: /foo/bar/../baz is correctly resolved before comparison.
      def path_contained_in?(path, container)
        # Resolve both paths to their canonical form
        resolved_path = begin
          File.realpath(path)
        rescue Errno::ENOENT
          File.expand_path(path)
        end
        resolved_container = begin
          File.realpath(container)
        rescue Errno::ENOENT
          File.expand_path(container)
        end

        # Use Pathname for proper path component comparison
        path_parts = Pathname.new(resolved_path).each_filename.to_a
        container_parts = Pathname.new(resolved_container).each_filename.to_a

        # The path must have at least as many components as the container
        return false if path_parts.size < container_parts.size

        # The first N components of path must match all container components
        path_parts.first(container_parts.size) == container_parts
      end

      # Find workspace root from $WORKSPACE
      def find
        ws = ENV['WORKSPACE'].to_s
        raise WorkspaceError, 'WORKSPACE environment variable not set' if ws.empty?
        raise WorkspaceError, "WORKSPACE directory does not exist: #{ws}" unless File.directory?(ws)
        ws
      end

      # Find all thread files in workspace (recursive traversal)
      def find_all_threads(ws)
        threads = []
        find_threads_recursive(ws, ws, threads)
        threads.sort
      end

      private

      # Recursively find threads, stopping at nested git repos
      def find_threads_recursive(dir, git_root, threads)
        # Collect from .threads at this level
        threads_dir = File.join(dir, '.threads')
        if File.directory?(threads_dir)
          Dir.glob(File.join(threads_dir, '*.md')).each do |path|
            threads << path unless path.include?('/archive/')
          end
        end

        # Recurse into subdirectories
        begin
          Dir.entries(dir).each do |name|
            next if name.start_with?('.')
            subdir = File.join(dir, name)
            next unless File.directory?(subdir)

            # Stop at nested git repos
            next if subdir != git_root && git_root?(subdir)

            find_threads_recursive(subdir, git_root, threads)
          end
        rescue Errno::EACCES, Errno::ENOENT
          # Skip unreadable directories
        end
      end

      public

      # Check if a directory is a git root (contains .git)
      def git_root?(path)
        git_path = File.join(path, '.git')
        File.exist?(git_path) && (File.directory?(git_path) || File.file?(git_path))
      end

      # Find git root for a path using git command
      def find_git_root_for_path(path)
        Dir.chdir(path) do
          output = `git rev-parse --show-toplevel 2>/dev/null`.strip
          return output unless output.empty?
        end
        nil
      rescue
        nil
      end

      # Find threads with direction and boundary options
      def find_threads_with_options(start_path, git_root, options)
        threads = []

        abs_start = File.expand_path(start_path)

        # Always collect threads at start_path
        collect_threads_at_path(abs_start, threads)

        # Search down (subdirectories)
        if options.has_down?
          max_depth = options.down_depth
          find_threads_down(abs_start, git_root, threads, 0, max_depth, false)
        end

        # Search up (parent directories)
        if options.has_up?
          max_depth = options.up_depth
          find_threads_up(abs_start, git_root, threads, 0, max_depth, false)
        end

        # Sort and deduplicate
        threads.uniq.sort
      end

      # Collect threads from .threads directory at the given path
      def collect_threads_at_path(dir, threads)
        threads_dir = File.join(dir, '.threads')
        return unless File.directory?(threads_dir)

        Dir.glob(File.join(threads_dir, '*.md')).each do |path|
          next if path.include?('/archive/')
          threads << path
        end
      end

      # Recursively find threads going down into subdirectories
      def find_threads_down(dir, git_root, threads, current_depth, max_depth, cross_git_boundaries)
        # Check depth limit (-1 means unlimited)
        return if max_depth >= 0 && current_depth >= max_depth

        begin
          entries = Dir.entries(dir)
        rescue Errno::EACCES, Errno::ENOENT
          return
        end

        entries.each do |name|
          next if name.start_with?('.')
          subdir = File.join(dir, name)
          next unless File.directory?(subdir)

          # Check git boundary
          if !cross_git_boundaries && subdir != git_root && git_root?(subdir)
            next
          end

          # Collect threads at this level
          collect_threads_at_path(subdir, threads)

          # Continue recursing
          find_threads_down(subdir, git_root, threads, current_depth + 1, max_depth, cross_git_boundaries)
        end
      end

      # Find threads going up into parent directories
      def find_threads_up(dir, git_root, threads, current_depth, max_depth, cross_git_boundaries)
        # Check depth limit (-1 means unlimited)
        return if max_depth >= 0 && current_depth >= max_depth

        parent = File.dirname(dir)
        return if parent == dir # reached root

        abs_parent = File.expand_path(parent)
        abs_git_root = File.expand_path(git_root)

        # Check git boundary: stop at git root unless crossing is allowed
        unless cross_git_boundaries
          return unless abs_parent.start_with?(abs_git_root)
        end

        # Collect threads at parent
        collect_threads_at_path(abs_parent, threads)

        # Continue up
        find_threads_up(abs_parent, git_root, threads, current_depth + 1, max_depth, cross_git_boundaries)
      end

      # Parse thread path to get git-relative path
      def parse_thread_relative_path(git_root, thread_path)
        abs_git_root = File.expand_path(git_root)
        abs_path = File.expand_path(thread_path)

        # Get path relative to git root
        rel = abs_path.sub("#{abs_git_root}/", '')
        return '.' if rel == abs_path

        # Extract the directory containing .threads
        # Pattern: <path>/.threads/file.md -> return <path>
        dir = File.dirname(rel)
        if dir.end_with?('/.threads')
          parent = File.dirname(dir)
          return '.' if parent == '.' || parent.empty?
          return parent
        end
        return '.' if dir == '.threads'

        '.'
      end

      # Generate unique 6-char hex ID
      def generate_id(ws)
        existing = find_all_threads(ws).map { |t| extract_id_from_path(t) }.compact.to_set

        10.times do
          id = SecureRandom.hex(3) # 6 chars
          return id unless existing.include?(id)
        end

        raise Threads::Error, 'could not generate unique ID after 10 attempts'
      end

      # Extract 6-char hex ID from filename
      def extract_id_from_path(path)
        filename = File.basename(path, '.md')
        match = filename.match(ID_PREFIX_RE)
        match ? match[1] : nil
      end

      # Extract name from filename (after ID prefix)
      def extract_name_from_path(path)
        filename = File.basename(path, '.md')
        match = filename.match(NAME_EXTRACT_RE)
        match ? match[1] : filename
      end

      # Convert title to kebab-case slug
      def slugify(title)
        s = title.downcase
        s = s.gsub(/[^a-z0-9]+/, '-')
        s = s.gsub(/-+/, '-')
        s = s.gsub(/^-|-$/, '')
        s
      end

      # Parse thread path to extract category, project, name
      def parse_thread_path(ws, path)
        # Canonicalize both paths to prevent path traversal
        resolved_path = begin
          File.realpath(path)
        rescue Errno::ENOENT
          File.expand_path(path)
        end
        resolved_ws = begin
          File.realpath(ws)
        rescue Errno::ENOENT
          File.expand_path(ws)
        end

        filename = File.basename(resolved_path, '.md')
        name = extract_name_from_path(resolved_path)
        name = filename if name.nil? || name.empty?

        # Compute relative path using canonical paths
        # Only strip the workspace prefix if the path is actually contained in it
        unless path_contained_in?(resolved_path, resolved_ws)
          return ['-', '-', name]
        end

        rel = resolved_path.sub("#{resolved_ws}/", '')

        # Check if workspace-level
        if rel.start_with?('.threads/')
          return ['-', '-', name]
        end

        parts = rel.split('/')
        category = parts[0] || '-'

        if parts.length >= 2 && parts[1] == '.threads'
          project = '-'
        elsif parts.length >= 3
          project = parts[1]
        else
          project = '-'
        end

        [category, project, name]
      end

      # Scope represents thread placement information
      Scope = Struct.new(:threads_dir, :category, :project, :level_desc)

      # Infer scope from path
      def infer_scope(ws, path)
        # Handle "." as PWD (current directory), not workspace root
        abs_path = if path == '.'
                     Dir.pwd
                   elsif path.start_with?('./')
                     # PWD-relative path
                     File.expand_path(path)
                   elsif File.absolute_path?(path)
                     path
                   elsif File.directory?(File.join(ws, path))
                     # Git-root-relative path
                     File.join(ws, path)
                   elsif File.directory?(path)
                     File.expand_path(path)
                   else
                     raise WorkspaceError, "path not found: #{path}"
                   end

        # Verify path exists
        raise WorkspaceError, "path not found or not a directory: #{path}" unless File.directory?(abs_path)

        # Must be within workspace (use secure path containment check)
        unless path_contained_in?(abs_path, ws)
          return Scope.new(
            File.join(ws, '.threads'),
            '-',
            '-',
            'workspace-level thread'
          )
        end

        # If abs_path equals ws, we're at workspace root
        abs_ws = File.expand_path(ws)
        abs_path_expanded = File.expand_path(abs_path)
        return Scope.new(File.join(ws, '.threads'), '-', '-', 'workspace-level thread') if abs_path_expanded == abs_ws

        rel = abs_path_expanded.sub("#{abs_ws}/", '')
        return Scope.new(File.join(ws, '.threads'), '-', '-', 'workspace-level thread') if rel.empty?

        parts = rel.split('/', 3)
        category = parts[0]
        project = parts[1] && !parts[1].empty? ? parts[1] : '-'

        if project == '-'
          Scope.new(
            File.join(ws, category, '.threads'),
            category,
            '-',
            "category-level thread (#{category})"
          )
        else
          Scope.new(
            File.join(ws, category, project, '.threads'),
            category,
            project,
            "project-level thread (#{category}/#{project})"
          )
        end
      end

      # Find thread by ID or name reference
      def find_by_ref(ws, ref)
        threads = find_all_threads(ws)

        # Fast path: exact 6-char hex ID
        if ref.match?(ID_ONLY_RE)
          threads.each do |t|
            return t if extract_id_from_path(t) == ref
          end
        end

        # Slow path: name matching
        substring_matches = []
        ref_lower = ref.downcase

        threads.each do |t|
          name = extract_name_from_path(t)

          # Exact name match
          return t if name == ref

          # Substring match (case-insensitive)
          substring_matches << t if name.downcase.include?(ref_lower)
        end

        return substring_matches[0] if substring_matches.length == 1

        if substring_matches.length > 1
          ids = substring_matches.map do |m|
            id = extract_id_from_path(m)
            name = extract_name_from_path(m)
            "#{id} (#{name})"
          end
          raise AmbiguousReference, "ambiguous reference '#{ref}' matches #{substring_matches.length} threads: #{ids.join(', ')}"
        end

        raise ThreadNotFound, "thread not found: #{ref}"
      end
    end
  end
end

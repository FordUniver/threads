# frozen_string_literal: true

require 'fileutils'
require 'securerandom'

module Threads
  # Workspace utilities for finding and navigating thread directories
  module Workspace
    class << self
      # Find workspace root from $WORKSPACE
      def find
        ws = ENV['WORKSPACE'].to_s
        raise 'WORKSPACE environment variable not set' if ws.empty?
        raise "WORKSPACE directory does not exist: #{ws}" unless File.directory?(ws)
        ws
      end

      # Find all thread files in workspace
      def find_all_threads(ws)
        patterns = [
          File.join(ws, '.threads', '*.md'),
          File.join(ws, '*', '.threads', '*.md'),
          File.join(ws, '*', '*', '.threads', '*.md')
        ]

        threads = []
        patterns.each do |pattern|
          Dir.glob(pattern).each do |path|
            # Skip archive directories
            next if path.include?('/archive/')

            threads << path
          end
        end

        threads.sort
      end

      # Generate unique 6-char hex ID
      def generate_id(ws)
        existing = find_all_threads(ws).map { |t| extract_id_from_path(t) }.compact.to_set

        10.times do
          id = SecureRandom.hex(3) # 6 chars
          return id unless existing.include?(id)
        end

        raise 'could not generate unique ID after 10 attempts'
      end

      # Extract 6-char hex ID from filename
      def extract_id_from_path(path)
        filename = File.basename(path, '.md')
        match = filename.match(/^([0-9a-f]{6})-/)
        match ? match[1] : nil
      end

      # Extract name from filename (after ID prefix)
      def extract_name_from_path(path)
        filename = File.basename(path, '.md')
        match = filename.match(/^[0-9a-f]{6}-(.+)$/)
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
        rel = path.sub("#{ws}/", '')
        filename = File.basename(path, '.md')
        name = extract_name_from_path(path)
        name = filename if name.nil? || name.empty?

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
        # Handle explicit "." for workspace level
        if path == '.'
          return Scope.new(
            File.join(ws, '.threads'),
            '-',
            '-',
            'workspace-level thread'
          )
        end

        # Resolve to absolute path
        abs_path = if File.absolute_path?(path)
                     path
                   elsif File.directory?(File.join(ws, path))
                     File.join(ws, path)
                   elsif File.directory?(path)
                     File.expand_path(path)
                   else
                     raise "path not found: #{path}"
                   end

        # Must be within workspace
        unless abs_path.start_with?(ws)
          return Scope.new(
            File.join(ws, '.threads'),
            '-',
            '-',
            'workspace-level thread'
          )
        end

        rel = abs_path.sub("#{ws}/", '')
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
        if ref.match?(/^[0-9a-f]{6}$/)
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
          raise "ambiguous reference '#{ref}' matches #{substring_matches.length} threads: #{ids.join(', ')}"
        end

        raise "thread not found: #{ref}"
      end
    end
  end
end

# frozen_string_literal: true

require 'digest'
require 'time'

module Threads
  # Section manipulation utilities
  module Section
    class << self
      # Extract section content between ## Name and next ## or EOF
      def extract(content, name)
        pattern = /(?:^|\n)## #{Regexp.escape(name)}\n(.*?)(?=\n## |\z)/m
        match = content.match(pattern)
        return '' unless match

        match[1].strip
      end

      # Replace section content
      def replace(content, name, new_content)
        pattern = /((?:^|\n)## #{Regexp.escape(name)}\n)(.*?)((?=\n## )|\z)/m
        return content unless content.match?(pattern)

        content.gsub(pattern) do
          prefix = ::Regexp.last_match(1)
          suffix = ::Regexp.last_match(3)
          "#{prefix}\n#{new_content}\n\n#{suffix}"
        end
      end

      # Append to section
      def append(content, name, addition)
        section_content = extract(content, name).strip
        new_content = section_content.empty? ? addition : "#{section_content}\n#{addition}"
        replace(content, name, new_content)
      end

      # Ensure section exists, placing before another section
      def ensure_section(content, name, before)
        return content if content.match?(/^## #{Regexp.escape(name)}$/m)

        before_pattern = /(^## #{Regexp.escape(before)})/m
        if content.match?(before_pattern)
          return content.gsub(before_pattern, "## #{name}\n\n\\1")
        end

        # If before section doesn't exist, append at end
        content + "\n## #{name}\n\n"
      end

      # Generate 4-char hash for an item
      def generate_hash(text)
        data = "#{text}#{Time.now.to_f}"
        Digest::MD5.hexdigest(data)[0, 4]
      end

      # Insert log entry with timestamp
      def insert_log_entry(content, entry)
        today = Time.now.strftime('%Y-%m-%d')
        timestamp = Time.now.strftime('%H:%M')
        bullet_entry = "- **#{timestamp}** #{entry}"
        heading = "### #{today}"

        # Check if today's heading exists
        if content.match?(/^### #{Regexp.escape(today)}$/m)
          # Insert after today's heading
          pattern = /(^### #{Regexp.escape(today)}\n)/m
          return content.gsub(pattern, "\\1\n#{bullet_entry}\n")
        end

        # Check if Log section exists
        if content.match?(/^## Log$/m)
          # Insert new heading after ## Log
          return content.gsub(/^## Log$/m, "## Log\n\n#{heading}\n\n#{bullet_entry}")
        end

        # No Log section - append one
        content + "\n## Log\n\n#{heading}\n\n#{bullet_entry}\n"
      end

      # Add note to Notes section with hash comment
      def add_note(content, text)
        content = ensure_section(content, 'Notes', 'Todo')
        hash = generate_hash(text)
        note_entry = "- #{text}  <!-- #{hash} -->"

        # Insert at top of Notes section
        pattern = /(^## Notes\n)/m
        new_content = content.gsub(pattern, "\\1\n#{note_entry}\n")

        [new_content, hash]
      end

      # Add todo item to Todo section
      def add_todo_item(content, text)
        hash = generate_hash(text)
        todo_entry = "- [ ] #{text}  <!-- #{hash} -->"

        # Insert at top of Todo section
        pattern = /(^## Todo\n)/m
        new_content = content.gsub(pattern, "\\1\n#{todo_entry}\n")

        [new_content, hash]
      end

      # Count items matching hash prefix in section
      def count_matching_items(content, section, hash)
        lines = content.split("\n")
        in_section = false
        hash_pattern = "<!-- #{hash}"
        count = 0

        lines.each do |line|
          if line.start_with?("## #{section}")
            in_section = true
          elsif line.start_with?('## ')
            in_section = false
          end

          count += 1 if in_section && line.include?(hash_pattern)
        end

        count
      end

      # Remove line by hash in section
      def remove_by_hash(content, section, hash)
        lines = content.split("\n")
        in_section = false
        hash_pattern = "<!-- #{hash}"
        found = false

        result = lines.reject do |line|
          if line.start_with?("## #{section}")
            in_section = true
          elsif line.start_with?('## ')
            in_section = false
          end

          if in_section && line.include?(hash_pattern) && !found
            found = true
            true
          else
            false
          end
        end

        raise "no item with hash '#{hash}' found" unless found

        result.join("\n")
      end

      # Edit item by hash in section
      def edit_by_hash(content, section, hash, new_text)
        lines = content.split("\n")
        in_section = false
        hash_pattern = "<!-- #{hash}"
        found = false

        result = lines.map do |line|
          if line.start_with?("## #{section}")
            in_section = true
          elsif line.start_with?('## ')
            in_section = false
          end

          if in_section && line.include?(hash_pattern) && !found
            found = true
            # Extract hash from line and rebuild
            match = line.match(/<!--\s*([a-f0-9]{4})\s*-->/)
            if match
              "- #{new_text}  <!-- #{match[1]} -->"
            else
              line
            end
          else
            line
          end
        end

        raise "no item with hash '#{hash}' found" unless found

        result.join("\n")
      end

      # Set todo item checked state by hash
      def set_todo_checked(content, hash, checked)
        lines = content.split("\n")
        in_todo = false
        hash_pattern = "<!-- #{hash}"
        found = false

        result = lines.map do |line|
          if line.start_with?('## Todo')
            in_todo = true
          elsif line.start_with?('## ')
            in_todo = false
          end

          if in_todo && line.include?(hash_pattern) && !found
            found = true
            if checked
              line.sub('- [ ]', '- [x]')
            else
              line.sub('- [x]', '- [ ]')
            end
          else
            line
          end
        end

        raise "no item with hash '#{hash}' found" unless found

        result.join("\n")
      end
    end
  end
end

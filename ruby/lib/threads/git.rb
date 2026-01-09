# frozen_string_literal: true

module Threads
  # Git utilities
  module Git
    class << self
      # Check if file has uncommitted changes
      def has_changes?(ws, rel_path)
        # Check unstaged changes
        system("git -C #{shell_escape(ws)} diff --quiet -- #{shell_escape(rel_path)} 2>/dev/null")
        return true unless $?.success?

        # Check staged changes
        system("git -C #{shell_escape(ws)} diff --cached --quiet -- #{shell_escape(rel_path)} 2>/dev/null")
        return true unless $?.success?

        # Check if untracked
        return true unless tracked?(ws, rel_path)

        false
      end

      # Check if file is tracked by git
      def tracked?(ws, rel_path)
        system("git -C #{shell_escape(ws)} ls-files --error-unmatch #{shell_escape(rel_path)} >/dev/null 2>&1")
        $?.success?
      end

      # Check if file exists in HEAD
      def exists_in_head?(ws, rel_path)
        ref = "HEAD:#{rel_path}"
        system("git -C #{shell_escape(ws)} cat-file -e #{shell_escape(ref)} 2>/dev/null")
        $?.success?
      end

      # Stage files
      def add(ws, *files)
        files_str = files.map { |f| shell_escape(f) }.join(' ')
        output = `git -C #{shell_escape(ws)} add #{files_str} 2>&1`
        raise "git add failed: #{output}" unless $?.success?
      end

      # Create commit
      def commit(ws, files, message)
        # Stage files
        add(ws, *files)

        # Commit
        files_str = files.map { |f| shell_escape(f) }.join(' ')
        output = `git -C #{shell_escape(ws)} commit -m #{shell_escape(message)} #{files_str} 2>&1`
        raise "git commit failed: #{output}" unless $?.success?
      end

      # Pull with rebase and push
      def push(ws)
        # Pull with rebase
        output = `git -C #{shell_escape(ws)} pull --rebase 2>&1`
        raise "git pull --rebase failed: #{output}" unless $?.success?

        # Push
        output = `git -C #{shell_escape(ws)} push 2>&1`
        raise "git push failed: #{output}" unless $?.success?
      end

      # Auto-commit: stage, commit, and push
      def auto_commit(ws, file, message)
        rel_path = file.sub("#{ws}/", '')
        commit(ws, [rel_path], message)

        begin
          push(ws)
        rescue StandardError => e
          warn "WARNING: git push failed (commit succeeded): #{e.message}"
        end
      end

      # Generate commit message for thread changes
      def generate_commit_message(ws, files)
        added = []
        modified = []
        deleted = []

        files.each do |file|
          rel_path = file.sub("#{ws}/", '')
          name = File.basename(file, '.md')

          if exists_in_head?(ws, rel_path)
            if File.exist?(file)
              modified << name
            else
              deleted << name
            end
          else
            added << name
          end
        end

        total = added.length + modified.length + deleted.length

        if total == 1
          return "threads: add #{extract_id(added[0])}" if added.length == 1
          return "threads: update #{extract_id(modified[0])}" if modified.length == 1

          return "threads: remove #{extract_id(deleted[0])}"
        end

        if total <= 3
          all_names = added + modified + deleted
          ids = all_names.map { |n| extract_id(n) }
          action = if added.length == total
                     'add'
                   elsif deleted.length == total
                     'remove'
                   else
                     'update'
                   end
          return "threads: #{action} #{ids.join(' ')}"
        end

        action = if added.length == total
                   'add'
                 elsif deleted.length == total
                   'remove'
                 else
                   'update'
                 end
        "threads: #{action} #{total} threads"
      end

      private

      # Extract ID prefix from filename
      def extract_id(name)
        return name[0, 6] if name.length >= 6 && name[0, 6].match?(/^[0-9a-f]{6}$/)

        name
      end

      # Shell escape a string
      def shell_escape(str)
        "'" + str.gsub("'", "'\\''") + "'"
      end
    end
  end
end

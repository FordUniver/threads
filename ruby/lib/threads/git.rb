# frozen_string_literal: true

require 'open3'

module Threads
  # Git utilities
  module Git
    class << self
      # Check if file has uncommitted changes
      def has_changes?(ws, rel_path)
        # Check unstaged changes
        _out, status = Open3.capture2('git', '-C', ws, 'diff', '--quiet', '--', rel_path)
        return true unless status.success?

        # Check staged changes
        _out, status = Open3.capture2('git', '-C', ws, 'diff', '--cached', '--quiet', '--', rel_path)
        return true unless status.success?

        # Check if untracked
        return true unless tracked?(ws, rel_path)

        false
      end

      # Check if file is tracked by git
      def tracked?(ws, rel_path)
        _out, status = Open3.capture2('git', '-C', ws, 'ls-files', '--error-unmatch', rel_path)
        status.success?
      end

      # Check if file exists in HEAD
      def exists_in_head?(ws, rel_path)
        ref = "HEAD:#{rel_path}"
        _out, status = Open3.capture2('git', '-C', ws, 'cat-file', '-e', ref)
        status.success?
      end

      # Stage files
      def add(ws, *files)
        output, status = Open3.capture2e('git', '-C', ws, 'add', *files)
        raise GitError, "git add failed: #{output}" unless status.success?
      end

      # Create commit
      def commit(ws, files, message)
        # Stage files
        add(ws, *files)

        # Commit
        output, status = Open3.capture2e('git', '-C', ws, 'commit', '-m', message, *files)
        raise GitError, "git commit failed: #{output}" unless status.success?
      end

      # Pull with rebase and push
      def push(ws)
        # Pull with rebase
        output, status = Open3.capture2e('git', '-C', ws, 'pull', '--rebase')
        raise GitError, "git pull --rebase failed: #{output}" unless status.success?

        # Push
        output, status = Open3.capture2e('git', '-C', ws, 'push')
        raise GitError, "git push failed: #{output}" unless status.success?
      end

      # Auto-commit: stage, commit, and push
      def auto_commit(ws, file, message)
        rel_path = file.sub("#{ws}/", '')
        commit(ws, [rel_path], message)

        begin
          push(ws)
        rescue GitError => e
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
    end
  end
end

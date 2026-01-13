# frozen_string_literal: true

require 'json'
require 'time'
require 'io/wait'

module Threads
  # Command implementations
  module Commands
    class << self
      # List threads
      def list(ws, opts = {})
        recursive = opts[:recursive]
        all = opts[:all]
        search = opts[:search]
        status_filter = opts[:status]
        json_output = opts[:json]
        path_filter = opts[:path]

        category_filter = nil
        project_filter = nil

        # Parse path filter
        if path_filter && !path_filter.empty?
          path_check = File.join(ws, path_filter)
          if File.directory?(path_check)
            parts = path_filter.split('/', 2)
            category_filter = parts[0]
            project_filter = parts[1] if parts.length > 1
          else
            # Treat as search filter
            search = path_filter
          end
        end

        threads = Workspace.find_all_threads(ws)
        results = []

        threads.each do |path|
          begin
            t = Thread.parse(path)
          rescue Threads::ParseError, Errno::ENOENT
            next
          end

          category, project, name = Workspace.parse_thread_path(ws, path)
          status = t.status
          base_status = t.base_status

          # Category filter
          next if category_filter && category != category_filter

          # Project filter
          next if project_filter && project != project_filter

          # Non-recursive: only threads at current hierarchy level
          unless recursive
            if project_filter
              # At project level, show all
            elsif category_filter
              # At category level: only show category-level threads
              next if project != '-'
            else
              # At workspace level: only show workspace-level threads
              next if category != '-'
            end
          end

          # Status filter
          if status_filter && !status_filter.empty?
            status_list = status_filter.split(',')
            next unless status_list.include?(base_status)
          elsif !all && t.terminal?
            next
          end

          # Search filter
          if search && !search.empty?
            search_lower = search.downcase
            name_lower = name.downcase
            title_lower = (t.name || '').downcase
            desc_lower = (t.desc || '').downcase

            unless name_lower.include?(search_lower) ||
                   title_lower.include?(search_lower) ||
                   desc_lower.include?(search_lower)
              next
            end
          end

          # Use title if available, else humanize name
          title = t.name
          title = name.tr('-', ' ') if title.nil? || title.empty?

          results << {
            id: t.id,
            status: base_status,
            category: category,
            project: project,
            name: name,
            title: title,
            desc: t.desc || ''
          }
        end

        if json_output
          puts JSON.pretty_generate(results)
          return
        end

        output_table(results, ws, category_filter, project_filter, status_filter, all, recursive)
      end

      # Create new thread
      def new_thread(ws, opts = {})
        path = opts[:path] || '.'
        title = opts[:title]
        status = opts[:status] || 'idea'
        desc = opts[:desc] || ''
        body_content = opts[:body] || ''

        raise ArgumentError, 'title is required' if title.nil? || title.empty?

        # Validate status before creating thread
        Threads.validate_status!(status)

        # Warn if no description
        warn 'Warning: No --desc provided. Add one with: threads update <id> --desc "..."' if desc.empty?

        # Slugify title
        slug = Workspace.slugify(title)
        raise ArgumentError, 'title produces empty slug' if slug.empty?

        # Read body from stdin if not provided and stdin has data
        if body_content.empty? && stdin_has_data?
          body_content = $stdin.read
        end

        # Determine scope
        scope = Workspace.infer_scope(ws, path)

        # Generate ID
        id = Workspace.generate_id(ws)

        # Ensure threads directory exists
        FileUtils.mkdir_p(scope.threads_dir)

        # Build file path
        filename = "#{id}-#{slug}.md"
        thread_path = File.join(scope.threads_dir, filename)

        raise Threads::Error, "thread already exists: #{thread_path}" if File.exist?(thread_path)

        # Generate content
        today = Time.now.strftime('%Y-%m-%d')
        timestamp = Time.now.strftime('%H:%M')

        content = "---\n"
        content += "id: #{id}\n"
        content += "name: #{title}\n"
        content += "desc: #{desc}\n"
        content += "status: #{status}\n"
        content += "---\n\n"

        if body_content && !body_content.empty?
          content += body_content
          content += "\n" unless body_content.end_with?("\n")
          content += "\n"
        end

        content += "## Todo\n\n"
        content += "## Log\n\n"
        content += "### #{today}\n\n"
        content += "- **#{timestamp}** Created thread.\n"

        File.write(thread_path, content)

        rel_path = thread_path.sub("#{ws}/", '')
        puts "Created #{scope.level_desc}: #{id}"
        puts "  → #{rel_path}"

        warn "Hint: Add body with: echo \"content\" | threads body #{id} --set" if body_content.empty?
        puts "Note: Thread #{id} has uncommitted changes. Use 'threads commit #{id}' when ready."
      end

      # Read thread content
      def read(ws, ref)
        file = Workspace.find_by_ref(ws, ref)
        print File.read(file)
      end

      # Print thread file path
      def path(ws, ref)
        file = Workspace.find_by_ref(ws, ref)
        puts File.absolute_path(file)
      end

      # Change thread status
      def status(ws, ref, new_status)
        # Validate status before applying
        Threads.validate_status!(new_status)

        file = Workspace.find_by_ref(ws, ref)
        t = Thread.parse(file)
        old_status = t.status

        t.set_field('status', new_status)
        t.write

        puts "Status changed: #{old_status} → #{new_status} (#{file})"
        puts "Note: Thread #{ref} has uncommitted changes. Use 'threads commit #{ref}' when ready."
      end

      # Update thread title/desc
      def update(ws, ref, opts = {})
        title = opts[:title]
        desc = opts[:desc]

        raise ArgumentError, 'specify --title and/or --desc' if (title.nil? || title.empty?) && (desc.nil? || desc.empty?)

        file = Workspace.find_by_ref(ws, ref)
        t = Thread.parse(file)

        if title && !title.empty?
          t.set_field('name', title)
          puts "Title updated: #{title}"
        end

        if desc && !desc.empty?
          t.set_field('desc', desc)
          puts "Description updated: #{desc}"
        end

        t.write
        puts "Updated: #{file}"
        puts "Note: Thread #{ref} has uncommitted changes. Use 'threads commit #{ref}' when ready."
      end

      # Edit body section
      def body(ws, ref, opts = {})
        set_mode = opts[:set]
        append_mode = opts[:append]

        # Default to set mode
        set_mode = true if !set_mode && !append_mode

        # Read content from stdin
        content = ''
        if stdin_has_data?
          content = $stdin.read
        end

        raise ArgumentError, 'no content provided (use stdin)' if content.empty?

        file = Workspace.find_by_ref(ws, ref)
        t = Thread.parse(file)

        if set_mode
          t.content = Section.replace(t.content, 'Body', content)
        else
          t.content = Section.append(t.content, 'Body', content)
        end

        t.write

        mode = append_mode ? 'append' : 'set'
        puts "Body #{mode}: #{file}"
        puts "Note: Thread #{ref} has uncommitted changes. Use 'threads commit #{ref}' when ready."
      end

      # Manage notes
      def note(ws, ref, action, *args)
        file = Workspace.find_by_ref(ws, ref)
        t = Thread.parse(file)

        case action
        when 'add'
          text = args[0]
          raise ArgumentError, 'usage: threads note <id> add "text"' if text.nil? || text.empty?

          t.content, hash = Section.add_note(t.content, text)
          t.content = Section.insert_log_entry(t.content, "Added note: #{text}")
          puts "Added note: #{text} (id: #{hash})"

        when 'edit'
          hash = args[0]
          new_text = args[1]
          raise ArgumentError, 'usage: threads note <id> edit <hash> "new text"' if hash.nil? || new_text.nil?

          count = Section.count_matching_items(t.content, 'Notes', hash)
          raise Threads::ThreadNotFound, "no note with hash '#{hash}' found" if count == 0
          raise Threads::AmbiguousReference, "ambiguous hash '#{hash}' matches #{count} notes" if count > 1

          t.content = Section.edit_by_hash(t.content, 'Notes', hash, new_text)
          t.content = Section.insert_log_entry(t.content, "Edited note #{hash}")
          puts "Edited note #{hash}"

        when 'remove'
          hash = args[0]
          raise ArgumentError, 'usage: threads note <id> remove <hash>' if hash.nil?

          count = Section.count_matching_items(t.content, 'Notes', hash)
          raise Threads::ThreadNotFound, "no note with hash '#{hash}' found" if count == 0
          raise Threads::AmbiguousReference, "ambiguous hash '#{hash}' matches #{count} notes" if count > 1

          t.content = Section.remove_by_hash(t.content, 'Notes', hash)
          t.content = Section.insert_log_entry(t.content, "Removed note #{hash}")
          puts "Removed note #{hash}"

        else
          raise ArgumentError, "unknown action '#{action}'. Use: add, edit, remove"
        end

        t.write
        puts "Note: Thread #{ref} has uncommitted changes. Use 'threads commit #{ref}' when ready."
      end

      # Manage todo items
      def todo(ws, ref, action, *args)
        file = Workspace.find_by_ref(ws, ref)
        t = Thread.parse(file)

        case action
        when 'add'
          text = args[0]
          raise ArgumentError, 'usage: threads todo <id> add "item text"' if text.nil? || text.empty?

          t.content, hash = Section.add_todo_item(t.content, text)
          puts "Added to Todo: #{text} (id: #{hash})"

        when 'check', 'complete', 'done'
          hash = args[0]
          raise ArgumentError, 'usage: threads todo <id> check <hash>' if hash.nil?

          count = Section.count_matching_items(t.content, 'Todo', hash)
          raise Threads::ThreadNotFound, "no unchecked item with hash '#{hash}' found" if count == 0
          raise Threads::AmbiguousReference, "ambiguous hash '#{hash}' matches #{count} items" if count > 1

          t.content = Section.set_todo_checked(t.content, hash, true)
          puts "Checked item #{hash}"

        when 'uncheck'
          hash = args[0]
          raise ArgumentError, 'usage: threads todo <id> uncheck <hash>' if hash.nil?

          count = Section.count_matching_items(t.content, 'Todo', hash)
          raise Threads::ThreadNotFound, "no checked item with hash '#{hash}' found" if count == 0
          raise Threads::AmbiguousReference, "ambiguous hash '#{hash}' matches #{count} items" if count > 1

          t.content = Section.set_todo_checked(t.content, hash, false)
          puts "Unchecked item #{hash}"

        when 'remove'
          hash = args[0]
          raise ArgumentError, 'usage: threads todo <id> remove <hash>' if hash.nil?

          count = Section.count_matching_items(t.content, 'Todo', hash)
          raise Threads::ThreadNotFound, "no item with hash '#{hash}' found" if count == 0
          raise Threads::AmbiguousReference, "ambiguous hash '#{hash}' matches #{count} items" if count > 1

          t.content = Section.remove_by_hash(t.content, 'Todo', hash)
          puts "Removed item #{hash}"

        else
          raise ArgumentError, "unknown action '#{action}'. Use: add, check, uncheck, remove"
        end

        t.write
        puts "Note: Thread #{ref} has uncommitted changes. Use 'threads commit #{ref}' when ready."
      end

      # Add log entry
      def log(ws, ref, entry = nil)
        # Read entry from stdin if not provided
        if entry.nil? || entry.empty?
          entry = $stdin.read if stdin_has_data?
        end

        raise ArgumentError, 'no log entry provided' if entry.nil? || entry.empty?

        file = Workspace.find_by_ref(ws, ref)
        t = Thread.parse(file)

        t.content = Section.insert_log_entry(t.content, entry.strip)
        t.write

        puts "Logged to: #{file}"
        puts "Note: Thread #{ref} has uncommitted changes. Use 'threads commit #{ref}' when ready."
      end

      # Resolve thread
      def resolve(ws, ref)
        file = Workspace.find_by_ref(ws, ref)
        t = Thread.parse(file)
        old_status = t.status

        t.set_field('status', 'resolved')
        t.content = Section.insert_log_entry(t.content, 'Resolved.')
        t.write

        puts "Resolved: #{old_status} → resolved (#{file})"
        puts "Note: Thread #{ref} has uncommitted changes. Use 'threads commit #{ref}' when ready."
      end

      # Reopen thread
      def reopen(ws, ref, opts = {})
        new_status = opts[:status] || 'active'

        # Validate status before applying
        Threads.validate_status!(new_status)

        file = Workspace.find_by_ref(ws, ref)
        t = Thread.parse(file)
        old_status = t.status

        t.set_field('status', new_status)
        t.content = Section.insert_log_entry(t.content, 'Reopened.')
        t.write

        puts "Reopened: #{old_status} → #{new_status} (#{file})"
        puts "Note: Thread #{ref} has uncommitted changes. Use 'threads commit #{ref}' when ready."
      end

      # Remove thread
      def remove(ws, ref)
        file = Workspace.find_by_ref(ws, ref)
        t = Thread.parse(file)
        name = t.name
        rel_path = file.sub("#{ws}/", '')

        was_tracked = Git.tracked?(ws, rel_path)

        File.delete(file)
        puts "Removed: #{file}"

        unless was_tracked
          puts 'Note: Thread was never committed to git, no commit needed.'
          return
        end

        puts 'Note: To commit this removal, run:'
        puts "  git -C \"$WORKSPACE\" add \"#{rel_path}\" && git -C \"$WORKSPACE\" commit -m \"threads: remove '#{name}'\""
      end

      # Move thread
      def move(ws, ref, new_path)
        src_file = Workspace.find_by_ref(ws, ref)
        scope = Workspace.infer_scope(ws, new_path)

        # Ensure dest .threads/ exists
        FileUtils.mkdir_p(scope.threads_dir)

        # Move file
        filename = File.basename(src_file)
        dest_file = File.join(scope.threads_dir, filename)

        raise Threads::Error, "thread already exists at destination: #{dest_file}" if File.exist?(dest_file)

        FileUtils.mv(src_file, dest_file)

        rel_dest = dest_file.sub("#{ws}/", '')
        puts "Moved to #{scope.level_desc}"
        puts "  → #{rel_dest}"
        puts 'Note: Use --commit to commit this move'
      end

      # Show pending git changes
      def git(ws)
        threads = Workspace.find_all_threads(ws)
        modified = []

        threads.each do |t|
          rel_path = t.sub("#{ws}/", '')
          modified << rel_path if Git.has_changes?(ws, rel_path)
        end

        if modified.empty?
          puts 'No pending thread changes.'
          return
        end

        puts 'Pending thread changes:'
        modified.each { |f| puts "  #{f}" }
        puts
        puts 'Suggested:'
        puts "  git add #{modified.join(' ')} && git commit -m \"threads: update\" && git push"
      end

      # Commit thread changes
      def commit(ws, opts = {})
        pending = opts[:pending]
        ids = opts[:ids] || []
        message = opts[:message]

        files = []

        if pending
          # Collect all thread files with uncommitted changes
          threads = Workspace.find_all_threads(ws)
          threads.each do |t|
            rel_path = t.sub("#{ws}/", '')
            files << t if Git.has_changes?(ws, rel_path)
          end
        else
          raise ArgumentError, 'provide thread IDs or use --pending' if ids.empty?

          ids.each do |id|
            file = Workspace.find_by_ref(ws, id)
            rel_path = file.sub("#{ws}/", '')
            unless Git.has_changes?(ws, rel_path)
              puts "No changes in thread: #{id}"
              next
            end
            files << file
          end
        end

        if files.empty?
          puts 'No threads to commit.'
          return
        end

        # Generate commit message if not provided
        if message.nil? || message.empty?
          message = Git.generate_commit_message(ws, files)
          puts "Generated message: #{message}"
        end

        # Stage and commit
        rel_paths = files.map { |f| f.sub("#{ws}/", '') }
        Git.commit(ws, rel_paths, message)

        puts "Committed #{files.length} thread(s)."
        $stderr.puts "Note: Changes committed locally. Push with 'git push' when ready."
      end

      # Validate thread files
      def validate(ws, opts = {})
        path = opts[:path]

        files = if path && !path.empty?
                  abs_path = File.absolute_path?(path) ? path : File.join(ws, path)
                  [abs_path]
                else
                  Workspace.find_all_threads(ws)
                end

        errors = 0

        files.each do |file|
          rel_path = file.sub("#{ws}/", '')
          issues = []

          begin
            t = Thread.parse(file)

            issues << 'missing name/title field' if t.name.nil? || t.name.to_s.empty?

            if t.status.nil? || t.status.to_s.empty?
              issues << 'missing status field'
            elsif !Threads.valid_status?(t.status)
              issues << "invalid status '#{t.base_status}'"
            end
          rescue Threads::ParseError, Errno::ENOENT => e
            issues << "parse error: #{e.message}"
          end

          if issues.empty?
            puts "OK: #{rel_path}"
          else
            puts "WARN: #{rel_path}: #{issues.join(', ')}"
            errors += 1
          end
        end

        raise Threads::Error, "#{errors} validation error(s)" if errors > 0
      end

      # Show stats
      def stats(ws, opts = {})
        recursive = opts[:recursive]
        path_filter = opts[:path]

        category_filter = nil
        project_filter = nil

        if path_filter && !path_filter.empty?
          path_check = File.join(ws, path_filter)
          if File.directory?(path_check)
            parts = path_filter.split('/', 2)
            category_filter = parts[0]
            project_filter = parts[1] if parts.length > 1
          end
        end

        threads = Workspace.find_all_threads(ws)
        counts = Hash.new(0)
        total = 0

        threads.each do |path|
          category, project, = Workspace.parse_thread_path(ws, path)

          # Category filter
          next if category_filter && category != category_filter

          # Project filter
          next if project_filter && project != project_filter

          # Non-recursive: only threads at current hierarchy level
          unless recursive
            if project_filter
              # At project level, count all
            elsif category_filter
              next if project != '-'
            else
              next if category != '-'
            end
          end

          begin
            t = Thread.parse(path)
            status = t.base_status
            status = '(none)' if status.nil? || status.empty?
            counts[status] += 1
            total += 1
          rescue Threads::ParseError, Errno::ENOENT
            next
          end
        end

        # Build scope description
        level_desc, path_suffix = if project_filter && category_filter
                                    ['project-level', " (#{category_filter}/#{project_filter})"]
                                  elsif category_filter
                                    ['category-level', " (#{category_filter})"]
                                  else
                                    ['workspace-level', '']
                                  end

        recursive_suffix = recursive ? ' (including nested)' : ''

        puts "Stats for #{level_desc} threads#{path_suffix}#{recursive_suffix}"
        puts

        if total == 0
          puts 'No threads found.'
          puts 'Hint: use -r to include nested categories/projects' unless recursive
          return
        end

        # Sort by count descending
        sorted = counts.sort_by { |_, c| -c }

        puts '| Status     | Count |'
        puts '|------------|-------|'
        sorted.each do |status, count|
          puts "| #{status.ljust(10)} | #{count.to_s.rjust(5)} |"
        end
        puts '|------------|-------|'
        puts "| #{'Total'.ljust(10)} | #{total.to_s.rjust(5)} |"
      end

      private

      # Check if stdin has data available without blocking
      def stdin_has_data?
        return false if $stdin.tty?

        # Use IO.select with 0 timeout to check without blocking
        IO.select([$stdin], nil, nil, 0) != nil
      end

      # Truncate string
      def truncate(s, max)
        return s if s.length <= max

        s[0, max - 1] + '…'
      end

      # Output table format
      def output_table(results, ws, category_filter, project_filter, status_filter, all, recursive)
        # Build header description
        level_desc, path_suffix = if project_filter && category_filter
                                    ['project-level', " (#{category_filter}/#{project_filter})"]
                                  elsif category_filter
                                    ['category-level', " (#{category_filter})"]
                                  else
                                    ['workspace-level', '']
                                  end

        status_desc = if status_filter && !status_filter.empty?
                        status_filter
                      elsif all
                        ''
                      else
                        'active'
                      end

        recursive_suffix = recursive ? ' (including nested)' : ''

        if !status_desc.empty?
          puts "Showing #{results.length} #{status_desc} #{level_desc} threads#{path_suffix}#{recursive_suffix}"
        else
          puts "Showing #{results.length} #{level_desc} threads#{path_suffix} (all statuses)#{recursive_suffix}"
        end
        puts

        if results.empty?
          puts 'Hint: use -r to include nested categories/projects' unless recursive
          return
        end

        # Print table header
        puts format('%-6s %-10s %-18s %-22s %s', 'ID', 'STATUS', 'CATEGORY', 'PROJECT', 'NAME')
        puts format('%-6s %-10s %-18s %-22s %s', '--', '------', '--------', '-------', '----')

        results.each do |t|
          category = truncate(t[:category], 16)
          project = truncate(t[:project], 20)
          puts format('%-6s %-10s %-18s %-22s %s', t[:id], t[:status], category, project, t[:title])
        end
      end
    end
  end
end

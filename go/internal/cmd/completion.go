package cmd

import (
	"os"

	"github.com/spf13/cobra"
)

var completionCmd = &cobra.Command{
	Use:   "completion [bash|zsh|fish|powershell]",
	Short: "Generate shell completion script",
	Long: `Generate shell completion script for threads.

To load completions:

Bash:
  $ source <(threads completion bash)
  # Or add to ~/.bashrc:
  $ threads completion bash > ~/.bash_completion.d/threads

Zsh:
  $ source <(threads completion zsh)
  # Or add to fpath:
  $ threads completion zsh > "${fpath[1]}/_threads"

Fish:
  $ threads completion fish | source
  # Or persist:
  $ threads completion fish > ~/.config/fish/completions/threads.fish

PowerShell:
  PS> threads completion powershell | Out-String | Invoke-Expression
  # Or add to profile:
  PS> threads completion powershell >> $PROFILE
`,
	DisableFlagsInUseLine: true,
	ValidArgs:             []string{"bash", "zsh", "fish", "powershell"},
	Args:                  cobra.MatchAll(cobra.ExactArgs(1), cobra.OnlyValidArgs),
	RunE: func(cmd *cobra.Command, args []string) error {
		switch args[0] {
		case "bash":
			return rootCmd.GenBashCompletion(os.Stdout)
		case "zsh":
			return rootCmd.GenZshCompletion(os.Stdout)
		case "fish":
			return rootCmd.GenFishCompletion(os.Stdout, true)
		case "powershell":
			return rootCmd.GenPowerShellCompletionWithDesc(os.Stdout)
		}
		return nil
	},
}

func init() {
	rootCmd.AddCommand(completionCmd)
}

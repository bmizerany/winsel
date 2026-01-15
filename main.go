package main

import (
	"context"
	"errors"
	"flag"
	"fmt"
	"io"
	"log"
	"os"
	"os/exec"
	"strconv"
	"strings"

	"blake.io/winsel/kitty"
)

var debugLog *log.Logger

func init() {
	f, err := os.OpenFile("/tmp/winsel.log", os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0o644)
	if err != nil {
		log.Fatal(err)
	}
	debugLog = log.New(f, "winsel: ", log.LstdFlags|log.Lshortfile)
}

func main() {
	log.SetPrefix("winsel: ")
	if err := _main(); err != nil {
		debugLog.Printf("fatal error: %v", err)
		log.Fatal(err)
	}
}

var bin = func() string {
	path, err := os.Executable()
	if err != nil {
		panic(err)
	}
	return path
}()

// Environment variables
var (
	envKittyWindowID = os.Getenv("KITTY_WINDOW_ID")
	envHome, _       = os.UserHomeDir()
)

// abbrevHome replaces the home directory prefix with ~.
func abbrevHome(path string) string {
	if envHome != "" && strings.HasPrefix(path, envHome) {
		return "~" + strings.TrimPrefix(path, envHome)
	}
	return path
}

// makeMatchIDsQuery builds a kitty match expression for multiple window IDs.
// e.g. ["1", "2", "3"] -> "id:1 or id:2 or id:3"
func makeMatchIDsQuery(ids []string) string {
	var b strings.Builder
	for i, id := range ids {
		if i > 0 {
			b.WriteString(" or ")
		}
		b.WriteString("id:")
		b.WriteString(id)
	}
	return b.String()
}

func _main() error {
	ctx, cancel := context.WithCancelCause(context.Background())
	defer cancel(nil)

	var kc kitty.Client

	writeList := func(w io.Writer) error {
		st, err := kc.List(ctx, &kitty.ListParams{
			Match: "not id:" + envKittyWindowID,
		})
		if err != nil {
			return err
		}

		// Build OS Window ID to letter mapping (A, B, C, ...)
		// Letter is derived from OS Window ID so it's stable for the window's lifetime
		oswinLetter := make(map[int]string)
		for _, osw := range st.OSWindows {
			oswinLetter[osw.ID] = string(rune('A' + (osw.ID % 26)))
		}

		for win := range st.Windows() {
			cwd := abbrevHome(win.EffectiveCwd())

			// Only show title if different from cwd
			title := win.Title
			if title == cwd || title == win.EffectiveCwd() {
				title = ""
			}

			// Determine the main command:
			// 1. Use win.Cmdline if available (last reported command)
			// 2. Otherwise use the last ForegroundProcess (typically the parent/main command)
			cmd := win.Cmdline
			if cmd == "" && len(win.ForegroundProcesses) > 0 {
				last := win.ForegroundProcesses[len(win.ForegroundProcesses)-1]
				cmd = strings.Join(last.Cmdline, " ")
			}

			// Format: OS_LETTER:TAB_ID:WIN_ID (e.g., "A:105:233")
			winID := fmt.Sprintf("%s:%d:%d",
				oswinLetter[win.OSWindow.ID],
				win.Tab.ID,
				win.ID,
			)

			fmt.Fprintln(w, strings.Join([]string{
				strconv.Itoa(win.ID),
				winID,
				cwd,
				title,
				cmd,
			}, "\t"))
		}
		return nil
	}

	flag.Parse()

	switch flag.Arg(0) {
	case "":
		cmd := exec.CommandContext(ctx, "fzf",
			// Generate setup
			"--prompt=WINSEL> ",
			"--multi",
			"--ansi",
			"--layout=reverse",
			"--border",
			"--border-label-pos=bottom",
			"--border-label= ↵:focus ^a:all ^b:bg ^s:split ^y:yank ^c:close ^o:jump ^l:clear ^/:preview ",

			// Data format
			"--delimiter=\t",
			"--with-nth=2,3,4,5",
			"--accept-nth=1",

			"--no-select-1",
			"--no-exit-0",
			"--info=right",

			// Preview window
			"--preview="+bin+" show {1}",
			"--preview-window=right:60%:nowrap:~5:follow",

			// Keybindings
			"--bind=enter:execute-silent("+bin+" focus {+1})+accept",
			"--bind=ctrl-b:execute-silent("+bin+" bg {+1})+reload("+bin+" ls)",
			"--bind=ctrl-y:execute-silent("+bin+" yank {+1})",
			"--bind=ctrl-c:execute-silent("+bin+" close {+1})+reload("+bin+" ls)",
			"--bind=ctrl-s:execute-silent("+bin+" split {+1})+accept",

			"--bind=ctrl-a:select-all",
			"--bind=ctrl-o:jump",
			"--bind=ctrl-l:clear-query",
			"--bind=ctrl-/:toggle-preview",
		)

		stdin, err := cmd.StdinPipe()
		if err != nil {
			return err
		}
		defer stdin.Close()

		go func() {
			defer stdin.Close()
			if err := writeList(stdin); err != nil {
				debugLog.Printf("fzf stdin write error: %v", err)
				cancel(fmt.Errorf("write list: %w", err))
			}
		}()

		out, err := cmd.CombinedOutput()
		if err != nil {
			var exitErr *exec.ExitError
			if errors.As(err, &exitErr) && exitErr.ExitCode() == 130 {
				// ESC or Ctrl-C pressed; normal exit
				return nil
			}
			return fmt.Errorf("fzf failed: %s: %w: %s", cmd, err, out)
		}
		return context.Cause(ctx)
	case "ls":
		return writeList(os.Stdout)
	case "show":
		match := "id:" + flag.Arg(1)
		st, err := kc.List(ctx, &kitty.ListParams{Match: match})
		if err != nil {
			return err
		}
		for win := range st.Windows() {
			osLetter := string(rune('A' + (win.OSWindow.ID % 26)))
			fmt.Printf("ID: %s:%d:%d\n", osLetter, win.Tab.ID, win.ID)
			fmt.Printf("Dir: %s\n", abbrevHome(win.EffectiveCwd()))
			fmt.Printf("Cmd: %s\n", win.Cmdline)
			fmt.Println()
			break
		}
		text, err := kc.GetText(ctx, match, &kitty.GetTextParams{
			Extent: "all",
			ANSI:   true,
		})
		if err != nil {
			return err
		}
		fmt.Print(text)
		return nil
	case "yank":
		if flag.NArg() < 2 {
			return nil
		}
		var texts []string
		for _, winID := range flag.Args()[1:] {
			text, err := kc.GetText(ctx, "id:"+winID, &kitty.GetTextParams{
				Extent: "all",
			})
			if err != nil {
				debugLog.Printf("yank: get text error for %s: %v", winID, err)
				return err
			}
			texts = append(texts, text)
		}
		// TODO(bmizerany): use some universal clipboard library instead of pbcopy
		pbcopy := exec.CommandContext(ctx, "pbcopy")
		pbcopy.Stdin = strings.NewReader(strings.Join(texts, "\n"))
		return pbcopy.Run()
	case "focus":
		if flag.NArg() < 2 {
			return nil
		}
		if flag.NArg() == 2 {
			return kc.FocusWindow(ctx, "id:"+flag.Arg(1))
		}
		// Multiple windows: detach first to new tab, rest to same tab
		debugLog.Printf("focus: grouping %d windows", flag.NArg()-1)
		err := kc.DetachWindow(ctx, &kitty.DetachWindowParams{
			Match:     "id:" + flag.Arg(1),
			TargetTab: "new",
		})
		if err != nil {
			debugLog.Printf("focus: detach error: %v", err)
			return err
		}
		if flag.NArg() > 2 {
			err = kc.DetachWindow(ctx, &kitty.DetachWindowParams{
				Match:     makeMatchIDsQuery(flag.Args()[2:]),
				TargetTab: "id:" + flag.Arg(1),
			})
			if err != nil {
				debugLog.Printf("focus: detach error: %v", err)
				return err
			}
		}
		return kc.FocusWindow(ctx, "id:"+flag.Arg(1))
	case "bg":
		debugLog.Printf("bg: args=%v", flag.Args()[1:])
		if flag.NArg() < 2 {
			debugLog.Printf("bg: no windows selected, returning")
			return nil
		}
		// Check if a "BG" tab already exists (list all, can't filter since
		// kitty errors when no tabs match)
		st, err := kc.List(ctx, nil)
		if err != nil {
			debugLog.Printf("bg: list error: %v", err)
			return err
		}
		var bgTabID int
		for _, osw := range st.OSWindows {
			for _, tab := range osw.Tabs {
				if tab.Title == "BG" {
					bgTabID = tab.ID
					break
				}
			}
			if bgTabID != 0 {
				break
			}
		}
		p := &kitty.DetachWindowParams{
			Match: makeMatchIDsQuery(flag.Args()[1:]),
		}
		if bgTabID != 0 {
			debugLog.Printf("bg: found BG tab id=%d", bgTabID)
			p.TargetTab = "id:" + strconv.Itoa(bgTabID)
		} else {
			debugLog.Printf("bg: creating new BG tab")
			p.TargetTab = "new"
			p.TabTitle = "BG"
		}
		debugLog.Printf("bg: detaching match=%s to target=%s", p.Match, p.TargetTab)
		return kc.DetachWindow(ctx, p)
	case "split":
		if flag.NArg() < 2 {
			return nil
		}
		for _, winID := range flag.Args()[1:] {
			err := kc.DetachWindow(ctx, &kitty.DetachWindowParams{
				Match:     "id:" + winID,
				TargetTab: "new",
			})
			if err != nil {
				return err
			}
		}
		return nil
	case "close":
		if flag.NArg() < 2 {
			return nil
		}
		return kc.CloseWindow(ctx, makeMatchIDsQuery(flag.Args()[1:]))
	default:
		return fmt.Errorf("unknown command: %q", flag.Arg(0))
	}
}

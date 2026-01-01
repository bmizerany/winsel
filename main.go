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
	"time"

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
		for win := range st.Windows() {
			fmt.Fprintln(w, strings.Join([]string{
				strconv.Itoa(win.ID),
				abbrevHome(win.EffectiveCwd()) + ":",
				win.Title,
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
			"--border-label= ↵:focus ^b:bg ^y:yank ^del:close ",

			// Data format
			"--delimiter=\t",
			"--with-nth=2,3",
			"--accept-nth=1",

			"--no-select-1",
			"--no-exit-0",
			"--info=right",

			// Preview window
			"--preview="+bin+" show {1}",
			"--preview-window=right:60%,nowrap,follow,~4",

			// Keybindings
			"--bind=enter:execute-silent("+bin+" focus {+1})+accept",
			"--bind=ctrl-b:execute-silent("+bin+" bg {+1})",
			"--bind=ctrl-y:execute-silent("+bin+" yank {+1})",
			"--bind=ctrl-delete:execute-silent("+bin+" close {+1})+reload("+bin+" ls)",

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
			fmt.Printf("Cmd: %s\n", win.Cmdline)
			fmt.Printf("Dir: %s\n", win.EffectiveCwd())
			fmt.Printf("Run: %s\n", time.Since(win.CreatedAtTime()).Truncate(time.Second))
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
		if flag.NArg() < 2 {
			return nil
		}
		return kc.DetachWindow(ctx, &kitty.DetachWindowParams{
			Match:     makeMatchIDsQuery(flag.Args()[1:]),
			TargetTab: "title:BG",
		})
	case "close":
		if flag.NArg() < 2 {
			return nil
		}
		return kc.CloseWindow(ctx, makeMatchIDsQuery(flag.Args()[1:]))
	default:
		return fmt.Errorf("unknown command: %q", flag.Arg(0))
	}
}

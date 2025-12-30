package main

import (
	"context"
	"flag"
	"fmt"
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
)

// matchIDs builds a kitty match expression for multiple window IDs.
// e.g. ["1", "2", "3"] -> "id:1 or id:2 or id:3"
func matchIDs(ids []string) string {
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

	flag.Parse()
	switch flag.Arg(0) {
	case "":
		// Close any existing winsel instances and mark this one
		go func() {
			kc.CloseWindow(ctx, "var:winsel")
			kc.SetUserVars(ctx, &kitty.SetUserVarsParams{
				Match: "id:" + envKittyWindowID,
				Var:   []string{"winsel=1"},
			})
		}()
		defer kc.SetUserVars(context.Background(), &kitty.SetUserVarsParams{
			Match: "id:" + envKittyWindowID,
			Var:   []string{"winsel"},
		})

		cmd := exec.CommandContext(ctx, "fzf",
			// Generate setup
			"--prompt=WINSEL> ",
			"--multi",
			"--ansi",
			"--layout=reverse",
			"--border",
			"--border-label-pos=bottom",
			"--border-label= ↵:focus b:bg n:here t:tab h:hsplit v:vsplit y:yank x:kill ",

			// Data format
			"--delimiter=\t",
			"--with-nth=2,3",
			"--accept-nth=1",

			"--no-select-1",
			"--no-exit-0",
			"--info=right",

			// Preview window
			"--preview="+bin+" preview {1}",
			"--preview-window=right:60%,nowrap,follow,~4",

			// Keybindings
			"--bind=enter:execute-silent("+bin+" focus {+1})+accept",
			"--bind=ctrl-b:execute-silent("+bin+" bg {+1})+accept",
			"--bind=ctrl-n:execute-silent("+bin+" vsplit {+1})+accept",
			"--bind=ctrl-t:execute-silent("+bin+" tab {+1})+accept",
			"--bind=ctrl-v:execute-silent("+bin+" vsplit {+1})+accept",
			"--bind=ctrl-h:execute-silent("+bin+" hsplit {+1})+accept",
			"--bind=ctrl-y:execute-silent("+bin+" yank {+1})",
			"--bind=ctrl-x:execute-silent("+bin+" kill {+1})+accept",
		)

		stdin, err := cmd.StdinPipe()
		if err != nil {
			return err
		}
		defer stdin.Close()

		go func() {
			defer stdin.Close()
			st, err := kc.List(ctx, &kitty.ListParams{
				Match: "not id:" + envKittyWindowID,
			})
			if err != nil {
				cancel(err)
				return
			}
			for win := range st.Windows() {
				fmt.Fprintln(stdin, strings.Join([]string{
					strconv.Itoa(win.ID),
					win.Title,
				}, "\t"))
			}
		}()

		out, err := cmd.CombinedOutput()
		if err != nil {
			return fmt.Errorf("fzf failed: %s: %w: %s", cmd, err, out)
		}
		return context.Cause(ctx)
	case "preview":
		match := "id:" + flag.Arg(1)
		st, err := kc.List(ctx, &kitty.ListParams{Match: match})
		if err != nil {
			return err
		}
		for win := range st.Windows() {
			fmt.Printf("Cmd: %s\n", win.Cmdline)
			fmt.Printf("Dir: %s\n", win.Cwd)
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
				Match:     matchIDs(flag.Args()[2:]),
				TargetTab: "id:" + flag.Arg(1),
			})
			if err != nil {
				debugLog.Printf("focus: detach error: %v", err)
				return err
			}
		}
		return kc.FocusWindow(ctx, "id:"+flag.Arg(1))
	case "bg":
		debugLog.Printf("bg command: args=%v", flag.Args()[1:])
		hasBGTab, err := func() (bool, error) {
			st, err := kc.List(ctx, &kitty.ListParams{
				MatchTab: "title:^BG$",
			})
			if err != nil {
				// kitty returns error when no tabs match - treat as "no BG tab"
				if strings.Contains(err.Error(), "No matching") {
					debugLog.Printf("no BG tab found")
					return false, nil
				}
				debugLog.Printf("list error: %v", err)
				return false, err
			}
			debugLog.Printf("list returned %d os windows", len(st.OSWindows))
			for range st.Windows() {
				debugLog.Printf("found BG tab")
				return true, nil
			}
			return false, nil
		}()
		if err != nil {
			return err
		}
		debugLog.Printf("hasBGTab=%v", hasBGTab)

		if !hasBGTab {
			debugLog.Printf("creating BG tab")
			winID, err := kc.Launch(ctx, &kitty.LaunchParams{
				Type:     "tab",
				TabTitle: "BG",
			})
			if err != nil {
				return err
			}
			debugLog.Printf("created BG tab with window %s", winID)
		}

		debugLog.Printf("detaching windows %v to BG tab", flag.Args()[1:])
		err = kc.DetachWindow(ctx, &kitty.DetachWindowParams{
			Match:     matchIDs(flag.Args()[1:]),
			TargetTab: "title:^BG$",
		})
		if err != nil {
			debugLog.Printf("detach error: %v", err)
			return err
		}
		return nil
	case "tab":
		if flag.NArg() < 2 {
			return nil
		}
		// Detach all windows to a new tab
		err := kc.DetachWindow(ctx, &kitty.DetachWindowParams{
			Match:     "id:" + flag.Arg(1),
			TargetTab: "new",
		})
		if err != nil {
			return err
		}
		if flag.NArg() > 2 {
			err = kc.DetachWindow(ctx, &kitty.DetachWindowParams{
				Match:     matchIDs(flag.Args()[2:]),
				TargetTab: "id:" + flag.Arg(1),
			})
			if err != nil {
				return err
			}
		}
		return kc.FocusWindow(ctx, "id:"+flag.Arg(1))
	case "hsplit", "vsplit":
		if flag.NArg() < 2 {
			return nil
		}
		// Detach windows to current tab (adds as splits)
		err := kc.DetachWindow(ctx, &kitty.DetachWindowParams{
			Match:     matchIDs(flag.Args()[1:]),
			TargetTab: "window_id:" + envKittyWindowID,
		})
		if err != nil {
			return err
		}
		if err := kc.FocusWindow(ctx, "id:"+flag.Arg(1)); err != nil {
			return err
		}
		// Update tab title to match focused window
		return kc.SetTabTitle(ctx, &kitty.SetTabTitleParams{
			Match: "window_id:" + envKittyWindowID,
		})
	case "kill":
		if flag.NArg() < 2 {
			return nil
		}
		return kc.CloseWindow(ctx, matchIDs(flag.Args()[1:]))
	default:
		return fmt.Errorf("unknown command: %q", flag.Arg(0))
	}
}

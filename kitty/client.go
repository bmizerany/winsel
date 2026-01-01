// Package kitty provides a client for interacting with the kitty terminal
// emulator's remote control features.
package kitty

import (
	"bufio"
	"bytes"
	"cmp"
	"context"
	"encoding/json"
	"iter"
	"net"
	"os"
	"strings"
	"time"

	"blake.io/wait"
)

type CommonParams struct {
	Match string `json:"match,omitempty"`
}

var protoVersion = []int{0, 14, 2}

// OSWindow represents a Kitty OS Window.
type OSWindow struct {
	ID   int
	Tabs []*Tab
}

// Tab represents a tab in an OS Window.
type Tab struct {
	ID        int
	IsActive  bool `json:"is_active"`
	IsFocused bool `json:"is_focused"`
	Title     string
	Windows   []*Window
}

// ForegroundProcess represents a process running in the foreground of a window.
type ForegroundProcess struct {
	Cwd     string   `json:"cwd"`
	Cmdline []string `json:"cmdline"`
	Pid     int      `json:"pid"`
}

// Window represents a window in a tab.
type Window struct {
	Tab                 *Tab
	ID                  int
	Cwd                 string               `json:"cwd"`
	Title               string               `json:"title"`
	Cmdline             string               `json:"last_reported_cmdline"`
	IsSelf              bool                 `json:"is_self"`
	Vars                Vars                 `json:"user_vars"`
	AtPrompt            bool                 `json:"at_prompt"`
	CreatedAt           int64                `json:"created_at"` // Unix nanoseconds
	ForegroundProcesses []*ForegroundProcess `json:"foreground_processes"`
}

func (w *Window) CreatedAtTime() time.Time {
	return time.Unix(0, w.CreatedAt)
}

// EffectiveCwd returns the cwd of the foreground process if available,
// otherwise falls back to the window's cwd.
func (w *Window) EffectiveCwd() string {
	if len(w.ForegroundProcesses) > 0 && w.ForegroundProcesses[0].Cwd != "" {
		return w.ForegroundProcesses[0].Cwd
	}
	return w.Cwd
}

type State struct {
	OSWindows []*OSWindow
}

// Windows returns a flattened sequence of all windows in s.
func (s *State) Windows() iter.Seq[*Window] {
	return func(yield func(*Window) bool) {
		for _, o := range s.OSWindows {
			for _, tab := range o.Tabs {
				for _, win := range tab.Windows {
					if !yield(win) {
						return
					}
				}
			}
		}
	}
}

// Vars represents user-defined kitty window vars.
// See 'kitty @ set-user-vars'.
type Vars map[string]string

// Get retrieves the value of the given key.
func (v Vars) Get(key string) string {
	return v[key]
}

// Has reports true if the given key exists; false otherwise.
func (v Vars) Has(key string) bool {
	_, ok := v[key]
	return ok
}

var envKittyListenOn = os.Getenv("KITTY_LISTEN_ON")

type connAndBuffer struct {
	br *bufio.Reader
	cn net.Conn
}

// Client is a kitty remote control client.
// Currently, it uses a single connection.
// Future versions may support a pool, but this works well enough for now.
type Client struct {
	To string // Same --to option as kitty @ commands. If empty, uses KITTY_LISTEN_ON env var.

	conns wait.List[connAndBuffer]
}

func (c *Client) baseURL() string {
	return cmp.Or(c.To, envKittyListenOn)
}

func (c *Client) dial(ctx context.Context) (*connAndBuffer, error) {
	var d net.Dialer
	cn, err := d.DialContext(ctx, "unix", strings.TrimPrefix(c.baseURL(), "unix:"))
	if err != nil {
		return nil, err
	}
	cc := &connAndBuffer{
		br: bufio.NewReader(cn),
		cn: cn,
	}
	return cc, nil
}

type matchParam struct {
	Match string `json:"match,omitempty"`
}

type ListParams struct {
	Match    string `json:"match,omitempty"`
	MatchTab string `json:"match_tab,omitempty"`
}

func (c *Client) List(ctx context.Context, p *ListParams) (*State, error) {
	vv, err := send[[]*OSWindow](ctx, c, "ls", p)
	if err != nil {
		return nil, err
	}
	return &State{OSWindows: vv}, nil
}

func (c *Client) FocusWindow(ctx context.Context, match string) error {
	_, err := send[noResponse](ctx, c, "focus-window", &CommonParams{Match: match})
	return err
}

func (c *Client) CloseWindow(ctx context.Context, match string) error {
	_, err := send[noResponse](ctx, c, "close-window", &CommonParams{Match: match})
	return err
}

type SetUserVarsParams struct {
	Match string   `json:"match,omitempty"`
	Var   []string `json:"var,omitempty"`
}

func (c *Client) SetUserVars(ctx context.Context, p *SetUserVarsParams) error {
	_, err := send[noResponse](ctx, c, "set-user-vars", p)
	return err
}

type SetTabTitleParams struct {
	Match string `json:"match,omitempty"`
	Title string `json:"title,omitempty"`
}

func (c *Client) SetTabTitle(ctx context.Context, p *SetTabTitleParams) error {
	_, err := send[noResponse](ctx, c, "set-tab-title", p)
	return err
}

type DetachWindowParams struct {
	Match     string `json:"match,omitempty"`
	TargetTab string `json:"target_tab,omitempty"`
	TabTitle  string `json:"tab_title,omitempty"`
}

func (c *Client) DetachWindow(ctx context.Context, p *DetachWindowParams) error {
	_, err := send[noResponse](ctx, c, "detach-window", p)
	return err
}

type LaunchParams struct {
	Type     string `json:"type,omitempty"`
	TabTitle string `json:"tab_title,omitempty"`
}

// Launch creates a new window or tab and returns the window ID.
func (c *Client) Launch(ctx context.Context, p *LaunchParams) (string, error) {
	return send[string](ctx, c, "launch", p)
}

type GetTextParams struct {
	Extent string `json:"extent,omitempty"`
	ANSI   bool   `json:"ansi,omitempty"`
}

func (c *Client) GetText(ctx context.Context, match string, p *GetTextParams) (string, error) {
	type payload struct {
		Match string `json:"match,omitempty"`
		*GetTextParams
	}
	return send[string](ctx, c, "get-text", payload{match, p})
}

type noResponse struct{}

// send is the Go version of:
//
//	echo -en '\eP@kitty-cmd{"cmd":"ls","version":[0,14,2]}\e\\' | socat - unix:/tmp/test | awk '{ print substr($0, 13, length($0) - 14) }' | jq -c '.data | fromjson' | jq .
//
// If Resp is the special type noResponse, kitty is instructed to not send a
// response and send will return without attempting to read one.
func send[Resp any](ctx context.Context, c *Client, cmd string, p any) (Resp, error) {
	var zero Resp
	_, noResponse := any(zero).(noResponse)
	data, err := json.Marshal(map[string]any{
		"cmd":         cmd,
		"version":     protoVersion,
		"no_response": noResponse,
		"payload":     p,
	})
	if err != nil {
		return zero, err
	}

	var in bytes.Buffer
	in.WriteString("\x1bP@kitty-cmd")
	in.Write(data)
	in.WriteString("\x1b\\")

	cc, err := c.conns.Take(ctx, func() connAndBuffer {
		cc, err := c.dial(ctx)
		if err != nil {
			panic(err)
		}
		return *cc
	})
	if err != nil {
		return zero, err
	}
	defer c.conns.Put(cc)

	stopc := make(chan struct{})
	stop := context.AfterFunc(ctx, func() {
		cc.cn.SetWriteDeadline(time.Now())
		cc.cn.SetReadDeadline(time.Now())
		close(stopc)
	})
	_, err = in.WriteTo(cc.cn)
	if !stop() {
		<-stopc
		cc.cn.SetWriteDeadline(time.Time{})
		cc.cn.SetReadDeadline(time.Time{})
	}
	if err != nil {
		return zero, err
	}

	if noResponse {
		return zero, nil
	}

	return readResponse[Resp](cc.br)
}

type Error struct {
	cr *commandResponse
}

func (e *Error) Error() string     { return e.cr.Error }
func (e *Error) Traceback() string { return e.cr.Traceback }

type commandResponse struct {
	OK        bool
	Data      string
	Error     string
	Traceback string `json:"tb"`
}

func readResponse[Resp any](r *bufio.Reader) (Resp, error) {
	var zero Resp

	r.Discard(12) // <ESC>P@kitty-cmd
	data, err := r.ReadBytes('\x1b')
	if err != nil {
		return zero, err
	}
	data = data[:len(data)-1] // remove trailing <ESC>
	r.Discard(1)              // discard trailing backslash from <ESC>\ terminator

	var cr *commandResponse
	err = json.Unmarshal(data, &cr)
	if err != nil {
		return zero, err
	}
	if !cr.OK {
		return zero, &Error{cr}
	}

	var resp Resp
	switch any(zero).(type) {
	case string:
		return any(cr.Data).(Resp), nil
	default:
		err := json.Unmarshal([]byte(cr.Data), &resp)
		if err != nil {
			return zero, err
		}
		return resp, nil
	}
}

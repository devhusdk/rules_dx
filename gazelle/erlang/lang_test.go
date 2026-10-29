package erlang

import (
	"testing"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
)

func TestParseAppSrc(t *testing.T) {
	got := ParseAppSrc([]byte(`%% {vsn, "0.0.0"}
{application, kv_store, [
    {description, "Key-value store."},
    {vsn, "1.2.3"},
    {mod, {kv_store_app, []}}
]}.
`))
	want := AppSrc{Name: "kv_store", Version: "1.2.3", Description: "Key-value store.", Module: "kv_store_app"}
	if got != want {
		t.Fatalf("ParseAppSrc = %+v, want %+v", got, want)
	}
}

func TestStdLib(t *testing.T) {
	for mod, app := range map[string]string{"lists": "stdlib", "gen_server": "stdlib", "crypto": "crypto", "ssl": "ssl"} {
		if got, ok := OTPApp(mod); !ok || got != app {
			t.Errorf("OTPApp(%q) = %q, %v, want %q", mod, got, ok, app)
		}
	}
	if IsStdLib("thoas") {
		t.Error("thoas is not an OTP module")
	}
	if !IsOTPApp("ssl") || IsOTPApp("thoas") {
		t.Error("IsOTPApp mismatch")
	}
}

func TestOwns(t *testing.T) {
	mk := func(kind, name string, srcs ...string) *rule.Rule {
		r := rule.NewRule(kind, name)
		r.SetAttr("srcs", srcs)
		return r
	}
	if !Owns(mk(AppKind, "erlang_app", "src/a.erl", "src/a.hrl")) {
		t.Error("generated app should be owned")
	}
	if Owns(mk(AppKind, "erlang_app", "src/a.erl", "other.erl")) {
		t.Error("foreign source should not be owned")
	}
	if Owns(mk(AppKind, "", "src/a.erl")) {
		t.Error("unnamed rule should not be owned")
	}
	if !Owns(mk(NifKind, "n", "c_src/a.c")) {
		t.Error("generated nif should be owned")
	}
}

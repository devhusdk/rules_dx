package erlang

import (
	"reflect"
	"testing"
)

func TestParseRefs(t *testing.T) {
	cases := []struct {
		name string
		src  string
		want Refs
	}{
		{"remote call", "-module(m).\nf() -> lists:map(fun(X) -> X end, []), foo:bar().\n", Refs{Modules: []string{"foo", "lists"}}},
		{"behaviour", "-behaviour(gen_server).\n-behavior(gen_event).\n", Refs{Modules: []string{"gen_event", "gen_server"}}},
		{"import", "-import(lists, [map/2]).\n", Refs{Modules: []string{"lists"}}},
		{"parse transform", "-compile({parse_transform, my_pt}).\n-compile([export_all, {parse_transform, other_pt}]).\n", Refs{Modules: []string{"my_pt", "other_pt"}}},
		{"include_lib", "-include_lib(\"ssl/include/ssl.hrl\").\n-include(\"local.hrl\").\n", Refs{Apps: []string{"ssl"}}},
		{"external fun", "f() -> fun foo:bar/1.\n", Refs{Modules: []string{"foo"}}},
		{"macro and variable modules", "f(M) -> M:run(), ?MODULE:go(), ok.\n", Refs{Modules: []string{}}},
		{"comments and strings", "% a:b()\nf() -> \"x:y()\", ok.\n", Refs{Modules: []string{}}},
		{"load_nif", "f() -> erlang:load_nif(\"./priv/fast\", 0).\n", Refs{Modules: []string{"erlang"}, NifNames: []string{"fast"}}},
		{"load_nif join", "f() -> erlang:load_nif(filename:join(P, \"fast\"), 0).\n", Refs{Modules: []string{"erlang", "filename"}, NifNames: []string{"fast"}}},
		{"parse error keeps earlier refs", "-behaviour(gen_server).\nf( -> foo:bar(.\n", Refs{Modules: []string{"gen_server"}}},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			got := ParseRefs([]byte(tc.src))
			norm := func(r Refs) Refs {
				if r.Modules == nil {
					r.Modules = []string{}
				}
				if r.Apps == nil {
					r.Apps = []string{}
				}
				if r.NifNames == nil {
					r.NifNames = []string{}
				}
				return r
			}
			if !reflect.DeepEqual(norm(got), norm(tc.want)) {
				t.Fatalf("ParseRefs = %+v, want %+v", got, tc.want)
			}
		})
	}
}

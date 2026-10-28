package powershell

import (
	"sort"
	"strings"
	"testing"

	"github.com/devhusdk/rules_dx/gazelle/common/treesitter"
)

type parseCase struct {
	name     string
	src      string
	manifest bool
	want     string
	// gap marks inputs the grammar cannot parse; only the legacy fallback
	// finds their imports.
	gap bool
}

var parseCases = []parseCase{
		{"bare", "Import-Module Greet\n", false, "Greet", false},
		{"lowercase command", "import-module greet -Force\n", false, "greet", false},
		{"named", "Import-Module -Force -Name 'Greet'\n", false, "Greet", false},
		{"named inline", "Import-Module -Name:Greet\n", false, "Greet", false},
		{"list", "Import-Module A, 'B',\"C\"\n", false, "A,B,C", false},
		{"value flags", "Import-Module Pester -MinimumVersion 5.7.1 -ErrorAction Stop\n", false, "Pester", false},
		{"flag first", "Import-Module -MinimumVersion 5.7.1 Pester\n", false, "Pester", false},
		{"join-path", "Import-Module (Join-Path $PSScriptRoot \"Greet.psm1\") -Force\n", false, "Greet", false},
		{"join-path manifest", "Import-Module (Join-Path $PSScriptRoot 'lib' 'Greet.psd1')\n", false, "Greet", false},
		{"interpolated path", "Import-Module \"$PSScriptRoot/Greet.psm1\"\n", false, "Greet", false},
		{"bare path", "Import-Module $PSScriptRoot\\Modules\\Greet.psd1\n", false, "Greet", false},
		{"variable", "Import-Module $name\nImport-Module -Name $mod\n", false, "", false},
		{"computed", "Import-Module \"$prefix.Greet\"\nImport-Module $(Get-Name)\n", false, "", false},
		{"binary", "Import-Module ./bin/Tool.dll\n", false, "", false},
		{"statements", "Import-Module A; Import-Module B | Out-Null\n", false, "A,B", false},
		{"continuation", "Import-Module `\n  -Name Greet `\n  -Force\n", false, "Greet", false},
		{"using", "using module Greet\nusing module ./lib/Other.psm1\nusing module @{ModuleName='Hash'; ModuleVersion='1.0'}\n", false, "Greet,Hash,Other", false},
		{"requires", "#Requires -Version 7.0 -Modules A, @{ ModuleName = 'B'; ModuleVersion = '1.0' }\n#requires -Modules 'C' -RunAsAdministrator\n", false, "A,B,C", false},
		{"fqn", "Import-Module -FullyQualifiedName @{ModuleName=\"Greet\"; ModuleVersion='1.0'}\n", false, "Greet", false},
		{"line comment", "# Import-Module Decoy\nWrite-Host 1 # Import-Module Decoy2\n", false, "", false},
		{"block comment", "<#\nImport-Module Decoy\n#Requires -Modules Decoy2\n#>\n", false, "", false},
		{"strings", "Write-Host \"Import-Module Decoy\"\n$x = 'using module Decoy2'\n", false, "", false},
		{"here-strings", "$a = @\"\nImport-Module Decoy\n\"@\n$b = @'\nImport-Module Decoy2\n'@\nImport-Module Real\n", false, "Real", false},
		{"indented requires is a comment", "if ($x) {\n  Write-Host 1 #Requires -Modules Decoy\n}\n", false, "", false},
		{"script block", "& { Import-Module Inner }\n", false, "Inner", false},
		{"dedupe and sort", "Import-Module B\nImport-Module A\nImport-Module B\n", false, "A,B", false},
		{"manifest", "@{\n  RootModule = 'Greet.psm1'\n  RequiredModules = @(\n    'A'\n    @{ ModuleName = 'B'; RequiredVersion = '2.0' },\n    \"C\"\n  )\n}\n", true, "A,B,C", false},
		{"manifest single", "@{ RequiredModules = 'A' }\n", true, "A", false},
		{"manifest decoy", "@{\n  # RequiredModules = @('Decoy')\n  Description = 'RequiredModules = Decoy2'\n}\n", true, "", false},
		{"manifest off", "@{ RequiredModules = @('A') }\n", false, "", false},
	{"inline quoted", "Import-Module -Name:'Q'\n", false, "Q", false},
	{"name list", "Import-Module -Name \"A\",\"B\"\n", false, "A,B", false},
	{"name array", "Import-Module -Name @(\"A\",\"B\") -MinimumVersion 5.7.1\n", false, "A,B", false},
	{"in function", "function F {\n  Import-Module Inner\n}\n", false, "Inner", false},
	{"in class", "class C {\n  [void] M() {\n    Import-Module Inner\n  }\n}\n", false, "Inner", false},
	{"assigned", "$null = Import-Module X -PassThru\n", false, "X", false},
	{"using namespace", "using namespace System.IO\n", false, "", false},
	{"crlf", "#requires -Modules A\r\nImport-Module B\r\n", false, "A,B", false},
	{"indented requires at line start", "if ($x) {\n  #Requires -Modules X\n}\n", false, "X", false},
	{"multi-line string", "$s = 'a\nImport-Module Decoy\nb'\n", false, "", false},
	{"bare ModuleName", "Import-Module -FullyQualifiedName @{ModuleName=Bare}\n", false, "Bare", false},
	{"truncated array", "Import-Module Before\n$x = @(\n", false, "Before", false},
	{"truncated here-string", "Import-Module Before\n$x = @\"\nImport-Module Decoy\n", false, "Before", false},
	{"truncated block", "Import-Module Before\nfunction F {\n  Import-Module Inner\n", false, "Before,Inner", false},
	{"empty", "", false, "", false},
	{"comma list gap", "Import-Module A,B\nImport-Module After\n", false, "A,After,B", true},
	{"name comma list gap", "Import-Module -Name A,B -Force\n", false, "A,B", true},
	{"requires comma list gap", "#Requires -Modules A,B\n", false, "A,B", true},
	{"analyzer gap", "try {\n    Import-Module -Name PSScriptAnalyzer, Pester -ErrorAction Stop\n    Import-Module After\n} catch {\n    throw\n}\n", false, "After,PSScriptAnalyzer,Pester", true},
}

func TestParseImports(t *testing.T) {
	for _, tc := range parseCases {
		t.Run(tc.name, func(t *testing.T) {
			if got := strings.Join(ParseImports([]byte(tc.src), tc.manifest), ","); got != tc.want {
				t.Errorf("ParseImports = %q, want %q", got, tc.want)
			}
		})
	}
}

// TestTreeImports runs the tree-sitter path alone, so coverage stays when the
// legacy fallback is deleted.
func TestTreeImports(t *testing.T) {
	for _, tc := range parseCases {
		if tc.gap {
			continue
		}
		t.Run(tc.name, func(t *testing.T) {
			set := map[string]bool{}
			treeImports([]byte(tc.src), tc.manifest, func(names []string) {
				for _, n := range names {
					set[n] = true
				}
			})
			var got []string
			for n := range set {
				got = append(got, n)
			}
			sort.Strings(got)
			if joined := strings.Join(got, ","); joined != tc.want {
				t.Errorf("tree imports = %q, want %q", joined, tc.want)
			}
		})
	}
}

// TestGrammarGapTripwire fails once the grammar parses unquoted comma lists.
func TestGrammarGapTripwire(t *testing.T) {
	if !treesitter.Parse(grammar(), []byte("Import-Module A,B\n")).HasError {
		t.Fatal("grammar fixed upstream (airbus-cert b03e6b0e); delete legacy.go, drop `gap`, close the follow-up issue")
	}
}

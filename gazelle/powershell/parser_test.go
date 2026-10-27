package powershell

import (
	"strings"
	"testing"
)

func TestParseImports(t *testing.T) {
	cases := []struct {
		name     string
		src      string
		manifest bool
		want     string
	}{
		{"bare", "Import-Module Greet\n", false, "Greet"},
		{"lowercase command", "import-module greet -Force\n", false, "greet"},
		{"named", "Import-Module -Force -Name 'Greet'\n", false, "Greet"},
		{"named inline", "Import-Module -Name:Greet\n", false, "Greet"},
		{"list", "Import-Module A, 'B',\"C\"\n", false, "A,B,C"},
		{"value flags", "Import-Module Pester -MinimumVersion 5.7.1 -ErrorAction Stop\n", false, "Pester"},
		{"flag first", "Import-Module -MinimumVersion 5.7.1 Pester\n", false, "Pester"},
		{"join-path", "Import-Module (Join-Path $PSScriptRoot \"Greet.psm1\") -Force\n", false, "Greet"},
		{"join-path manifest", "Import-Module (Join-Path $PSScriptRoot 'lib' 'Greet.psd1')\n", false, "Greet"},
		{"interpolated path", "Import-Module \"$PSScriptRoot/Greet.psm1\"\n", false, "Greet"},
		{"bare path", "Import-Module $PSScriptRoot\\Modules\\Greet.psd1\n", false, "Greet"},
		{"variable", "Import-Module $name\nImport-Module -Name $mod\n", false, ""},
		{"computed", "Import-Module \"$prefix.Greet\"\nImport-Module $(Get-Name)\n", false, ""},
		{"binary", "Import-Module ./bin/Tool.dll\n", false, ""},
		{"statements", "Import-Module A; Import-Module B | Out-Null\n", false, "A,B"},
		{"continuation", "Import-Module `\n  -Name Greet `\n  -Force\n", false, "Greet"},
		{"using", "using module Greet\nusing module ./lib/Other.psm1\nusing module @{ModuleName='Hash'; ModuleVersion='1.0'}\n", false, "Greet,Hash,Other"},
		{"requires", "#Requires -Version 7.0 -Modules A, @{ ModuleName = 'B'; ModuleVersion = '1.0' }\n#requires -Modules 'C' -RunAsAdministrator\n", false, "A,B,C"},
		{"fqn", "Import-Module -FullyQualifiedName @{ModuleName=\"Greet\"; ModuleVersion='1.0'}\n", false, "Greet"},
		{"line comment", "# Import-Module Decoy\nWrite-Host 1 # Import-Module Decoy2\n", false, ""},
		{"block comment", "<#\nImport-Module Decoy\n#Requires -Modules Decoy2\n#>\n", false, ""},
		{"strings", "Write-Host \"Import-Module Decoy\"\n$x = 'using module Decoy2'\n", false, ""},
		{"here-strings", "$a = @\"\nImport-Module Decoy\n\"@\n$b = @'\nImport-Module Decoy2\n'@\nImport-Module Real\n", false, "Real"},
		{"indented requires is a comment", "if ($x) {\n  Write-Host 1 #Requires -Modules Decoy\n}\n", false, ""},
		{"script block", "& { Import-Module Inner }\n", false, "Inner"},
		{"dedupe and sort", "Import-Module B\nImport-Module A\nImport-Module B\n", false, "A,B"},
		{"manifest", "@{\n  RootModule = 'Greet.psm1'\n  RequiredModules = @(\n    'A'\n    @{ ModuleName = 'B'; RequiredVersion = '2.0' },\n    \"C\"\n  )\n}\n", true, "A,B,C"},
		{"manifest single", "@{ RequiredModules = 'A' }\n", true, "A"},
		{"manifest decoy", "@{\n  # RequiredModules = @('Decoy')\n  Description = 'RequiredModules = Decoy2'\n}\n", true, ""},
		{"manifest off", "@{ RequiredModules = @('A') }\n", false, ""},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			if got := strings.Join(ParseImports([]byte(tc.src), tc.manifest), ","); got != tc.want {
				t.Errorf("ParseImports = %q, want %q", got, tc.want)
			}
		})
	}
}

# Emits the raw API facts of PowerShell sources as dx.powershell.api/1 JSON.
# It only parses: no source is imported, dot-sourced or invoked.
param(
    [Parameter(Mandatory)][string]$Sources,
    [string]$Manifest = '',
    [string]$ManifestRel = '',
    [Parameter(Mandatory)][string]$Out
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$PSModuleAutoLoadingPreference = 'None'

$Schema = 'dx.powershell.api/1'
# A using module statement whose module is not on this machine is not a source error.
$Tolerated = @('ModuleNotFoundDuringParse')

function Stop-Extract([string]$Rel, [int]$Line, [string]$Message) {
    [Console]::Error.WriteLine("docs_powershell: ${Rel}:${Line}: $Message")
    exit 1
}

function Read-Ast([string]$Path, [string]$Rel) {
    $tokens = $null
    $errors = $null
    $ast = [System.Management.Automation.Language.Parser]::ParseFile($Path, [ref]$tokens, [ref]$errors)
    foreach ($parseError in $errors) {
        if ($Tolerated -notcontains $parseError.ErrorId) {
            Stop-Extract $Rel $parseError.Extent.StartLineNumber $parseError.Message
        }
    }
    , @($ast, @($errors | Where-Object { $Tolerated -contains $_.ErrorId }))
}

function Get-AttributeName($Attribute) {
    $Attribute.TypeName.Name
}

function Test-Attribute($Attribute, [string[]]$Names) {
    $name = Get-AttributeName $Attribute
    foreach ($candidate in $Names) {
        if ($name -ieq $candidate) { return $true }
    }
    $false
}

function Get-ConstantValues($Expression) {
    # Returns the literal strings of one expression, or $null when it is not literal.
    if ($null -eq $Expression) { return , @() }
    if ($Expression -is [System.Management.Automation.Language.StringConstantExpressionAst]) {
        return , @($Expression.Value)
    }
    if ($Expression -is [System.Management.Automation.Language.ConstantExpressionAst]) {
        return , @([string]$Expression.Value)
    }
    if ($Expression -is [System.Management.Automation.Language.ArrayLiteralAst]) {
        $values = @()
        foreach ($element in $Expression.Elements) {
            $inner = Get-ConstantValues $element
            if ($null -eq $inner) { return $null }
            $values += $inner
        }
        return , $values
    }
    if ($Expression -is [System.Management.Automation.Language.ArrayExpressionAst] -or
        $Expression -is [System.Management.Automation.Language.ParenExpressionAst]) {
        $pipelines = if ($Expression -is [System.Management.Automation.Language.ArrayExpressionAst]) {
            @($Expression.SubExpression.Statements)
        } else {
            @($Expression.Pipeline)
        }
        $values = @()
        foreach ($statement in $pipelines) {
            if ($statement -isnot [System.Management.Automation.Language.PipelineAst] -or $statement.PipelineElements.Count -ne 1) { return $null }
            $element = $statement.PipelineElements[0]
            if ($element -isnot [System.Management.Automation.Language.CommandExpressionAst]) { return $null }
            $inner = Get-ConstantValues $element.Expression
            if ($null -eq $inner) { return $null }
            $values += $inner
        }
        return , $values
    }
    $null
}

function Get-AttributeStrings($Attribute) {
    $values = @()
    foreach ($argument in $Attribute.PositionalArguments) {
        if ($argument -is [System.Management.Automation.Language.TypeExpressionAst]) {
            $values += $argument.TypeName.FullName
            continue
        }
        $inner = Get-ConstantValues $argument
        if ($null -ne $inner) { $values += $inner }
    }
    , $values
}

function Get-NamedArgument($Attribute, [string]$Name) {
    foreach ($named in $Attribute.NamedArguments) {
        if ($named.ArgumentName -ieq $Name) { return $named }
    }
    $null
}

function Test-NamedTrue($Attribute, [string]$Name) {
    $named = Get-NamedArgument $Attribute $Name
    if ($null -eq $named) { return $false }
    if ($named.ExpressionOmitted) { return $true }
    $expression = $named.Argument
    if ($expression -is [System.Management.Automation.Language.VariableExpressionAst]) {
        return $expression.VariablePath.UserPath -ieq 'true'
    }
    if ($expression -is [System.Management.Automation.Language.ConstantExpressionAst]) {
        return [bool]$expression.Value
    }
    $false
}

function Get-NamedString($Attribute, [string]$Name) {
    $named = Get-NamedArgument $Attribute $Name
    if ($null -eq $named) { return $null }
    $values = Get-ConstantValues $named.Argument
    if ($null -eq $values -or $values.Count -ne 1) { return $null }
    [string]$values[0]
}

function Get-HelpFact($Function) {
    $help = $Function.GetHelpContent()
    if ($null -eq $help) { return $null }
    $parameters = [ordered]@{}
    $keys = [string[]]@($help.Parameters.Keys)
    [Array]::Sort($keys, [StringComparer]::Ordinal)
    foreach ($key in $keys) {
        $parameters[[string]$key] = [string]$help.Parameters[$key]
    }
    [ordered]@{
        synopsis    = [string]$help.Synopsis
        description = [string]$help.Description
        parameters  = $parameters
        examples    = @($help.Examples | ForEach-Object { [string]$_ })
        outputs     = @($help.Outputs | ForEach-Object { [string]$_ })
        notes       = [string]$help.Notes
    }
}

function Get-ParamFact($Parameter) {
    $type = $null
    $sets = @()
    $aliases = @()
    foreach ($attribute in $Parameter.Attributes) {
        if ($attribute -is [System.Management.Automation.Language.TypeConstraintAst]) {
            $type = $attribute.TypeName.FullName
            continue
        }
        if (Test-Attribute $attribute @('Parameter', 'System.Management.Automation.Parameter', 'ParameterAttribute')) {
            $position = $null
            $named = Get-NamedArgument $attribute 'Position'
            if ($null -ne $named -and $named.Argument -is [System.Management.Automation.Language.ConstantExpressionAst]) {
                $position = [int]$named.Argument.Value
            }
            $setName = Get-NamedString $attribute 'ParameterSetName'
            $sets += [ordered]@{
                name      = if ($null -eq $setName) { '__AllParameterSets' } else { $setName }
                mandatory = Test-NamedTrue $attribute 'Mandatory'
                position  = $position
                pipeline  = (Test-NamedTrue $attribute 'ValueFromPipeline') -or (Test-NamedTrue $attribute 'ValueFromPipelineByPropertyName')
            }
            continue
        }
        if (Test-Attribute $attribute @('Alias', 'AliasAttribute')) {
            $aliases += Get-AttributeStrings $attribute
        }
    }
    [ordered]@{
        name    = $Parameter.Name.VariablePath.UserPath
        type    = $type
        sets    = @($sets)
        aliases = @($aliases)
    }
}

function Get-FunctionFact($Function) {
    $binding = $null
    $outputTypes = @()
    $aliases = @()
    $params = @()
    $body = $Function.Body
    if ($null -ne $body.ParamBlock) {
        foreach ($attribute in $body.ParamBlock.Attributes) {
            if (Test-Attribute $attribute @('CmdletBinding', 'CmdletBindingAttribute')) {
                $positional = Get-NamedArgument $attribute 'PositionalBinding'
                $binding = [ordered]@{
                    default_set        = Get-NamedString $attribute 'DefaultParameterSetName'
                    positional_binding = ($null -eq $positional) -or (Test-NamedTrue $attribute 'PositionalBinding')
                }
            } elseif (Test-Attribute $attribute @('OutputType', 'OutputTypeAttribute')) {
                $outputTypes += Get-AttributeStrings $attribute
            } elseif (Test-Attribute $attribute @('Alias', 'AliasAttribute')) {
                $aliases += Get-AttributeStrings $attribute
            }
        }
        foreach ($parameter in $body.ParamBlock.Parameters) {
            $params += , (Get-ParamFact $parameter)
        }
    } elseif ($null -ne $Function.Parameters) {
        foreach ($parameter in $Function.Parameters) {
            $params += , (Get-ParamFact $parameter)
        }
    }
    [ordered]@{
        name           = $Function.Name
        line           = $Function.Extent.StartLineNumber
        help           = Get-HelpFact $Function
        cmdlet_binding = $binding
        output_types   = @($outputTypes)
        aliases        = @($aliases)
        params         = @($params)
    }
}

function Get-ExportFact($Ast) {
    $commands = $Ast.FindAll({
            param($node)
            $node -is [System.Management.Automation.Language.CommandAst] -and $node.GetCommandName() -ieq 'Export-ModuleMember'
        }, $true)
    if (@($commands).Count -eq 0) {
        return [ordered]@{ kind = 'none'; functions = @() }
    }
    $functions = @()
    foreach ($command in $commands) {
        $count = $command.CommandElements.Count
        $elements = if ($count -gt 1) { @($command.CommandElements)[1..($count - 1)] } else { @() }
        $index = 0
        $positional = $true
        while ($index -lt $elements.Count) {
            $element = $elements[$index]
            $index++
            if ($element -is [System.Management.Automation.Language.CommandParameterAst]) {
                $name = $element.ParameterName
                $value = $element.Argument
                if ($null -eq $value -and $index -lt $elements.Count -and
                    $elements[$index] -isnot [System.Management.Automation.Language.CommandParameterAst]) {
                    $value = $elements[$index]
                    $index++
                }
                if ('Function'.StartsWith($name, [StringComparison]::OrdinalIgnoreCase)) {
                    $values = Get-ConstantValues $value
                    if ($null -eq $values) { return [ordered]@{ kind = 'dynamic'; functions = @() } }
                    $functions += $values
                } elseif ($null -ne $value -and $null -eq (Get-ConstantValues $value)) {
                    return [ordered]@{ kind = 'dynamic'; functions = @() }
                }
                $positional = $false
                continue
            }
            if (-not $positional) {
                return [ordered]@{ kind = 'dynamic'; functions = @() }
            }
            $values = Get-ConstantValues $element
            if ($null -eq $values) { return [ordered]@{ kind = 'dynamic'; functions = @() } }
            $functions += $values
            $positional = $false
        }
    }
    [ordered]@{ kind = 'literal'; functions = @($functions) }
}

function ConvertTo-Fact($Value) {
    # Writes compact JSON without the Utility module, which is not loaded with autoloading off.
    if ($null -eq $Value) { return 'null' }
    if ($Value -is [bool]) { return $(if ($Value) { 'true' } else { 'false' }) }
    if ($Value -is [int] -or $Value -is [long]) { return [string]$Value }
    if ($Value -is [string]) { return '"' + [System.Web.HttpUtility]::JavaScriptStringEncode($Value) + '"' }
    if ($Value -is [System.Collections.IDictionary]) {
        $parts = foreach ($key in $Value.Keys) {
            '"' + [System.Web.HttpUtility]::JavaScriptStringEncode([string]$key) + '":' + (ConvertTo-Fact $Value[$key])
        }
        return '{' + (@($parts) -join ',') + '}'
    }
    if ($Value -is [System.Collections.IEnumerable]) {
        $parts = foreach ($item in $Value) { ConvertTo-Fact $item }
        return '[' + (@($parts) -join ',') + ']'
    }
    throw "cannot write a $($Value.GetType().FullName) as JSON"
}

function Get-ManifestFact([string]$Path, [string]$Rel) {
    $parsed = Read-Ast $Path $Rel
    $ast = $parsed[0]
    $table = $ast.Find({ param($node) $node -is [System.Management.Automation.Language.HashtableAst] }, $false)
    if ($null -eq $table) {
        Stop-Extract $Rel 1 'module manifest is not a hashtable'
    }
    $facts = [ordered]@{
        rel                 = $Rel
        root_module         = $null
        module_version      = $null
        functions_to_export = $null
        aliases_to_export   = $null
    }
    foreach ($pair in $table.KeyValuePairs) {
        $key = $pair.Item1
        if ($key -isnot [System.Management.Automation.Language.StringConstantExpressionAst]) { continue }
        $field = switch ($key.Value) {
            'RootModule' { 'root_module' }
            'ModuleToProcess' { 'root_module' }
            'ModuleVersion' { 'module_version' }
            'FunctionsToExport' { 'functions_to_export' }
            'AliasesToExport' { 'aliases_to_export' }
            default { $null }
        }
        if ($null -eq $field) { continue }
        try {
            $value = $pair.Item2.SafeGetValue()
        } catch {
            Stop-Extract $Rel $pair.Item2.Extent.StartLineNumber "$($key.Value) is not a constant value"
        }
        if ($field -eq 'functions_to_export' -or $field -eq 'aliases_to_export') {
            $names = @($value | ForEach-Object { [string]$_ })
            if ($names.Count -eq 1 -and $names[0] -eq '*') {
                $facts[$field] = '*'
            } else {
                $facts[$field] = $names
            }
        } else {
            $facts[$field] = [string]$value
        }
    }
    $facts
}

$files = @()
foreach ($row in [System.IO.File]::ReadAllLines($Sources)) {
    if ($row -eq '') { continue }
    $columns = $row.Split("`t")
    if ($columns.Count -ne 2) {
        Stop-Extract $Sources 1 "malformed sources line '$row'"
    }
    $path = $columns[0]
    $rel = $columns[1]
    $parsed = Read-Ast $path $rel
    $ast = $parsed[0]
    $functions = @()
    foreach ($statement in $ast.EndBlock.Statements) {
        if ($statement -is [System.Management.Automation.Language.FunctionDefinitionAst]) {
            $functions += , (Get-FunctionFact $statement)
        }
    }
    $files += , ([ordered]@{
            rel             = $rel
            functions       = @($functions)
            exports         = Get-ExportFact $ast
            unresolved_uses = @($parsed[1] | ForEach-Object { $_.Extent.Text })
        })
}

$manifestFacts = $null
if ($Manifest -ne '') {
    $manifestFacts = Get-ManifestFact $Manifest $ManifestRel
}

$document = [ordered]@{
    schema   = $Schema
    pwsh     = $PSVersionTable.PSVersion.ToString()
    manifest = $manifestFacts
    files    = @($files)
}
$json = ConvertTo-Fact $document
[System.IO.File]::WriteAllText($Out, $json + "`n", [System.Text.UTF8Encoding]::new($false))

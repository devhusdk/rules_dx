module FSharpLintRunner

open System
open System.IO
open System.Text.Json
open FSharp.Compiler.CodeAnalysis
open FSharp.Compiler.Diagnostics
open FSharp.Compiler.Text
open FSharpLint.Application

let private parse (argv: string[]) =
    let rec go config files args =
        match args with
        | "--config" :: path :: rest -> go (Some path) files rest
        | "--config" :: [] -> Error "--config needs a path"
        | file :: rest -> go config (file :: files) rest
        | [] -> Ok(config, List.rev files)

    go None [] (List.ofArray argv)

let private write path severity rule message (range: range) =
    let record =
        {| path = path
           severity = severity
           rule = rule
           message = message
           startLine = range.StartLine
           startColumn = range.StartColumn + 1
           endLine = range.EndLine
           endColumn = range.EndColumn + 1 |}

    Console.Out.WriteLine(JsonSerializer.Serialize record)

let private emit (path: string) (warning: FSharpLint.Framework.Suggestion.LintWarning) =
    write path "warning" warning.RuleIdentifier warning.Details.Message warning.Details.Range

let private syntaxErrors (checker: FSharpChecker) (file: string) =
    let options =
        { FSharpParsingOptions.Default with
            SourceFiles = [| file |] }

    let parsed =
        checker.ParseFile(file, SourceText.ofString (File.ReadAllText file), options)
        |> Async.RunSynchronously

    parsed.Diagnostics
    |> Array.filter (fun diagnostic -> diagnostic.Severity = FSharpDiagnosticSeverity.Error)

[<EntryPoint>]
let main argv =
    match Environment.GetEnvironmentVariable "FSHARPLINT_CWD" with
    | null
    | "" -> ()
    | work -> Environment.CurrentDirectory <- work

    match parse argv with
    | Error message ->
        eprintfn "fsharplint: %s" message
        2
    | Ok(_, []) ->
        eprintfn "fsharplint: no files to lint"
        2
    | Ok(config, files) ->
        let parameters =
            { Lint.OptionalLintParameters.Default with
                Configuration =
                    match config with
                    | Some path -> Lint.ConfigurationParam.FromFile path
                    | None -> Lint.ConfigurationParam.Default }

        let checker = FSharpChecker.Create()
        let mutable failed = false

        for file in files do
            let errors = if File.Exists file then syntaxErrors checker file else [||]

            if errors.Length > 0 then
                for error in errors do
                    write file "error" (sprintf "FS%04d" error.ErrorNumber) error.Message error.Range
            else
                match Lint.lintFile parameters file with
                | LintResult.Success warnings ->
                    for warning in warnings do
                        emit file warning
                | LintResult.Failure failure ->
                    failed <- true
                    eprintfn "fsharplint: %s: %s" file failure.Description

        if failed then 1 else 0

module FSharpLintRunner

open System
open System.IO
open System.Text.Json
open FSharp.Compiler.CodeAnalysis
open FSharp.Compiler.Diagnostics
open FSharp.Compiler.Text
open FSharpLint.Application

type private Options =
    { Config: string option
      Compile: string list
      Refs: string list
      Defines: string list
      LangVersion: string
      Files: string list }

let private parse (argv: string[]) =
    let rec go options args =
        match args with
        | "--config" :: path :: rest -> go { options with Config = Some path } rest
        | "--compile" :: path :: rest -> go { options with Compile = path :: options.Compile } rest
        | "--ref" :: path :: rest -> go { options with Refs = path :: options.Refs } rest
        | "--define" :: symbol :: rest -> go { options with Defines = symbol :: options.Defines } rest
        | "--langversion" :: version :: rest -> go { options with LangVersion = version } rest
        | flag :: [] when flag.StartsWith "--" -> Error(sprintf "%s needs a value" flag)
        | file :: rest -> go { options with Files = file :: options.Files } rest
        | [] ->
            Ok
                { options with
                    Compile = List.rev options.Compile
                    Refs = List.rev options.Refs
                    Defines = List.rev options.Defines
                    Files = List.rev options.Files }

    go
        { Config = None
          Compile = []
          Refs = []
          Defines = []
          LangVersion = ""
          Files = [] }
        (List.ofArray argv)

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

let private frameworkReferences (explicit: string list) =
    let named = explicit |> List.map Path.GetFileName |> Set.ofList
    let directory = Runtime.InteropServices.RuntimeEnvironment.GetRuntimeDirectory()

    Directory.GetFiles(directory, "*.dll")
    |> Array.filter (fun path ->
        let name = Path.GetFileName path

        not (named.Contains name)
        && (name.StartsWith "System."
            || name.StartsWith "Microsoft."
            || name = "netstandard.dll"
            || name = "mscorlib.dll"
            || name = "WindowsBase.dll"))
    |> List.ofArray

let private isError (diagnostic: FSharpDiagnostic) =
    diagnostic.Severity = FSharpDiagnosticSeverity.Error

let private lintAll (options: Options) =
    let parameters =
        { Lint.OptionalLintParameters.Default with
            Configuration =
                match options.Config with
                | Some path -> Lint.ConfigurationParam.FromFile path
                | None -> Lint.ConfigurationParam.Default }

    let checker = FSharpChecker.Create()

    let arguments =
        [| yield "--noframework"
           yield "--nologo"
           yield "--targetprofile:netcore"

           for symbol in options.Defines do
               yield "--define:" + symbol

           if options.LangVersion <> "" then
               yield "--langversion:" + options.LangVersion

           for reference in frameworkReferences options.Refs @ options.Refs do
               yield "-r:" + reference

           yield! options.Compile |]

    let project = checker.GetProjectOptionsFromCommandLineArgs("dx-fsharplint.fsproj", arguments)
    let projectResults = checker.ParseAndCheckProject project |> Async.RunSynchronously
    let mutable failed = false

    let mutable syntax = Set.empty

    for file in options.Files do
        let parsed, checkAnswer =
            checker.GetBackgroundCheckResultsForFileInProject(file, project)
            |> Async.RunSynchronously

        let errors = parsed.Diagnostics |> Array.filter isError

        if errors.Length > 0 then
            syntax <- syntax.Add(Path.GetFullPath file)

            for error in errors do
                write file "error" (sprintf "FS%04d" error.ErrorNumber) error.Message error.Range
        else
            let information =
                { Lint.ParsedFileInformation.Ast = parsed.ParseTree
                  Lint.ParsedFileInformation.Source = File.ReadAllText file
                  Lint.ParsedFileInformation.TypeCheckResults = Some checkAnswer
                  Lint.ParsedFileInformation.ProjectCheckResults = Some projectResults }

            match Lint.lintParsedFile parameters information file with
            | LintResult.Success warnings ->
                for warning in warnings do
                    emit file warning
            | LintResult.Failure failure ->
                failed <- true
                eprintfn "fsharplint: %s: %s" file failure.Description

    for error in projectResults.Diagnostics |> Array.filter isError do
        if not (syntax.Contains(Path.GetFullPath error.FileName)) then
            failed <- true
            eprintfn "fsharplint: %s: FS%04d: %s" error.FileName error.ErrorNumber error.Message

    failed

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
    | Ok { Files = [] } ->
        eprintfn "fsharplint: no files to lint"
        2
    | Ok { Compile = [] } ->
        eprintfn "fsharplint: no compile context; pass the target sources in compile order with --compile"
        2
    | Ok options ->
        let compiled = options.Compile |> List.map Path.GetFullPath |> Set.ofList

        match options.Files |> List.tryFind (fun file -> not (compiled.Contains(Path.GetFullPath file))) with
        | Some file ->
            eprintfn "fsharplint: %s is not in the compile context" file
            2
        | None -> if lintAll options then 1 else 0

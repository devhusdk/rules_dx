module Dx.Docs.FsharpApi.Program

open System
open System.Collections.Generic
open System.IO
open System.Linq
open System.Text
open System.Text.Encodings.Web
open System.Text.Json
open System.Text.RegularExpressions
open System.Xml.Linq
open FSharp.Compiler.Symbols
open FSharp.Formatting.ApiDocs

[<Literal>]
let Schema = "dx.dotnet-api/1"

exception Failure of string

type Context =
    { Assembly: string
      Dll: string
      Xml: string
      DepRefs: string list
      Srcs: string list }

type Item =
    { DocId: string
      Kind: string
      Name: string
      DisplayName: string
      Parent: string option
      Signature: string
      Summary: string
      Remarks: string
      Params: (string * string * string) list
      Returns: (string * string) option
      Examples: string list
      File: string
      Line: int
      Implements: string list }

let fail message = raise (Failure message)

let strings (root: JsonElement) (name: string) =
    [ for element in root.GetProperty(name).EnumerateArray() -> element.GetString() ]

let readContext (path: string) =
    use document = JsonDocument.Parse(File.ReadAllText path)
    let root = document.RootElement

    { Assembly = root.GetProperty("assembly").GetString()
      Dll = root.GetProperty("dll").GetString()
      Xml = root.GetProperty("xml").GetString()
      DepRefs = strings root "dep_refs"
      Srcs = strings root "srcs" }

let displayContext = FSharpDisplayContext.Empty.WithShortTypeNames true

let formatType (t: FSharpType) = t.Format displayContext

let typars (parameters: seq<FSharpGenericParameter>) =
    if Seq.isEmpty parameters then
        ""
    else
        "<" + String.Join(", ", parameters |> Seq.map (fun p -> "'" + p.Name)) + ">"

let shortCref (cref: string) =
    let body =
        if cref.Length > 2 && cref.[1] = ':' then
            cref.Substring 2
        else
            cref

    let body =
        match body.IndexOf '(' with
        | -1 -> body
        | index -> body.Substring(0, index)

    match body.LastIndexOf '.' with
    | -1 -> body
    | index -> body.Substring(index + 1)

let tidy (text: string) =
    let lines = text.Replace("\r\n", "\n").Split '\n' |> Array.map (fun line -> line.TrimEnd())
    let joined = String.Join("\n", lines)
    Regex.Replace(joined, "\n{3,}", "\n\n").Trim()

let rec render (nodes: seq<XNode>) : string =
    String.Join("", nodes |> Seq.map renderNode)

and renderNode (node: XNode) : string =
    match node with
    | :? XText as text -> Regex.Replace(text.Value, @"[ \t]*\r?\n[ \t]*", "\n")
    | :? XElement as element ->
        let attr (name: string) =
            match element.Attribute(XName.Get name) with
            | null -> ""
            | a -> a.Value

        match element.Name.LocalName with
        | "see"
        | "seealso" ->
            if attr "cref" <> "" then "`" + shortCref (attr "cref") + "`"
            elif attr "langword" <> "" then "`" + attr "langword" + "`"
            elif attr "href" <> "" then attr "href"
            else render (element.Nodes())
        | "paramref"
        | "typeparamref" -> "`" + attr "name" + "`"
        | "c" -> "`" + element.Value + "`"
        | "para" -> "\n\n" + render (element.Nodes()) + "\n\n"
        | "code" -> "\n\n```\n" + tidy element.Value + "\n```\n\n"
        | _ -> render (element.Nodes())
    | _ -> ""

let childText (parent: XElement) (name: string) =
    match parent.Element(XName.Get name) with
    | null -> ""
    | element -> tidy (render (element.Nodes()))

let paramDocs (parent: XElement) =
    [ for element in parent.Elements(XName.Get "param") do
          match element.Attribute(XName.Get "name") with
          | null -> ()
          | a -> yield a.Value, tidy (render (element.Nodes())) ]

type Docs =
    { Summary: string
      Remarks: string
      Examples: string list
      Params: (string * string) list
      Returns: string }

let readDocs (comment: ApiDocComment) =
    match comment.Xml with
    | Some xml ->
        { Summary = childText xml "summary"
          Remarks = childText xml "remarks"
          Examples = [ for e in xml.Elements(XName.Get "example") -> tidy (render (e.Nodes())) ]
          Params = paramDocs xml
          Returns = childText xml "returns" }
    | None ->
        { Summary = ""
          Remarks = ""
          Examples = []
          Params = []
          Returns = "" }

let docIdOf (sigText: string) (fallbackKind: string) (fullName: string) =
    if String.IsNullOrEmpty sigText then
        "X:" + fallbackKind + ":" + fullName
    else
        sigText

let locate (range: FSharp.Compiler.Text.range) (what: string) =
    if String.IsNullOrEmpty range.FileName || range.StartLine <= 0 then
        fail (sprintf "'%s' has no source location" what)

    range.FileName.Replace('\\', '/'), range.StartLine

let entityKind (e: FSharpEntity) =
    if e.IsFSharpAbbreviation then "abbreviation"
    elif e.IsFSharpModule then "module"
    elif e.IsFSharpUnion then "union"
    elif e.IsFSharpRecord then "record"
    elif e.IsEnum then "enum"
    elif e.IsInterface then "interface"
    elif e.IsDelegate then "delegate"
    elif e.IsValueType then "struct"
    else "class"

let entityDocId (e: FSharpEntity) =
    docIdOf e.XmlDocSig "T" (defaultArg e.TryFullName e.DisplayName)

let namespaceOf (e: FSharpEntity) =
    match e.Namespace with
    | Some ns when ns <> "" -> ns
    | _ -> "global"

let entityName (e: FSharpEntity) =
    if e.IsFSharpModule then e.CompiledName else e.DisplayName

let entitySignature (e: FSharpEntity) =
    if e.IsFSharpModule then
        "module " + e.DisplayName
    elif e.IsFSharpAbbreviation then
        "type " + e.DisplayName + typars e.GenericParameters + " = " + formatType e.AbbreviatedType
    else
        "type " + e.DisplayName + typars e.GenericParameters

let isSynthesized (v: FSharpMemberOrFunctionOrValue) = v.IsCompilerGenerated || v.IsUnionCaseTester

let isPublic (v: FSharpMemberOrFunctionOrValue) = v.Accessibility.IsPublic

let typeMemberVisible (v: FSharpMemberOrFunctionOrValue) =
    isPublic v
    && not (isSynthesized v)
    && not v.IsOverrideOrExplicitInterfaceImplementation
    && not v.IsEventAddMethod
    && not v.IsEventRemoveMethod
    && not v.IsPropertyGetterMethod
    && not v.IsPropertySetterMethod

let moduleMemberVisible (v: FSharpMemberOrFunctionOrValue) =
    isPublic v
    && not (isSynthesized v)
    && ((not v.IsMember && not v.IsActivePattern) || v.IsExtensionMember || v.IsActivePattern)

let memberVisible (owner: FSharpEntity) (v: FSharpMemberOrFunctionOrValue) =
    if owner.IsFSharpModule then moduleMemberVisible v else typeMemberVisible v

let fieldVisible (owner: FSharpEntity) (f: FSharpField) =
    not owner.IsFSharpUnion
    && not owner.IsFSharpModule
    && not owner.IsInterface
    && not owner.IsFSharpAbbreviation
    && not f.IsCompilerGenerated
    && f.Name <> "value__"
    && f.Accessibility.IsPublic
    && (not owner.IsEnum || f.IsLiteral)

let groupText (group: IList<FSharpParameter>) =
    let parts =
        [ for p in group ->
              let t = formatType p.Type
              let optional = if p.IsOptionalArg then "?" else ""

              match p.Name with
              | Some n -> optional + n + ": " + t
              | None -> t ]

    match parts with
    | [ "unit" ] -> "()"
    | _ -> "(" + String.Join(", ", parts) + ")"

let groupsText (v: FSharpMemberOrFunctionOrValue) =
    String.Join(" ", v.CurriedParameterGroups |> Seq.map groupText)

let parametersOf (v: FSharpMemberOrFunctionOrValue) (docs: Docs) =
    v.CurriedParameterGroups
    |> Seq.collect id
    |> Seq.mapi (fun index p ->
        let name = defaultArg p.Name ("arg" + string (index + 1))
        let doc = docs.Params |> List.tryFind (fun (n, _) -> n = name) |> Option.map snd
        name, formatType p.Type, defaultArg doc "")
    |> List.ofSeq

let returnsOf (v: FSharpMemberOrFunctionOrValue) (docs: Docs) =
    let t = formatType v.ReturnParameter.Type
    if t = "unit" then None else Some(t, docs.Returns)

let memberKind (owner: FSharpEntity) (v: FSharpMemberOrFunctionOrValue) =
    if v.CompiledName = ".ctor" then "constructor"
    elif owner.IsFSharpModule then
        if v.LiteralValue.IsSome then "const"
        elif v.CurriedParameterGroups.Count > 0 || v.FullType.IsFunctionType then "function"
        else "field"
    elif v.IsProperty then "property"
    elif v.IsEvent then "event"
    elif v.CompiledName.StartsWith "op_" then "operator"
    else "method"

let memberSignature (owner: FSharpEntity) (v: FSharpMemberOrFunctionOrValue) (kind: string) =
    let ret = formatType v.ReturnParameter.Type
    let generics = typars v.GenericParameters
    let args = if kind = "property" && v.CurriedParameterGroups |> Seq.forall (fun g -> g.Count = 0) then "" else groupsText v

    match kind with
    | "constructor" -> "new" + args
    | "function" -> "let " + v.DisplayName + generics + (if args = "" then "" else " " + args) + " : " + ret
    | "const" -> "[<Literal>] let " + v.DisplayName + " : " + ret
    | "field" -> "let " + v.DisplayName + " : " + ret
    | _ ->
        let access =
            if v.IsDispatchSlot && owner.IsInterface then "abstract "
            elif v.IsInstanceMember then "member this."
            else "static member "

        let suffix =
            if kind = "property" && v.HasSetterMethod then " with get, set"
            elif kind = "property" && not v.HasSetterMethod then " with get"
            else ""

        access + v.DisplayName + generics + args + " : " + ret + suffix

let memberName (owner: FSharpEntity) (v: FSharpMemberOrFunctionOrValue) =
    if v.CompiledName = ".ctor" then "new" else v.DisplayName

let fieldSignature (f: FSharpField) =
    (if f.IsMutable then "mutable " else "") + f.Name + ": " + formatType f.FieldType

let caseSignature (c: FSharpUnionCase) =
    let named (f: FSharpField) =
        if Regex.IsMatch(f.Name, @"^Item\d*$") then
            formatType f.FieldType
        else
            f.Name + ": " + formatType f.FieldType

    if c.Fields.Count = 0 then
        "| " + c.Name
    else
        "| " + c.Name + " of " + String.Join(" * ", c.Fields |> Seq.map named)

let enumSignature (f: FSharpField) =
    match f.LiteralValue with
    | Some value -> f.Name + " = " + Convert.ToString(value, Globalization.CultureInfo.InvariantCulture)
    | None -> f.Name

let namespaceItem (name: string) =
    { DocId = "N:" + name
      Kind = "namespace"
      Name = name
      DisplayName = name
      Parent = None
      Signature = "namespace " + name
      Summary = ""
      Remarks = ""
      Params = []
      Returns = None
      Examples = []
      File = ""
      Line = 0
      Implements = [] }

let frameworkPrefixes = [ "System."; "mscorlib"; "netstandard"; "Microsoft.Win32" ]

let walkModel (model: ApiDocModel) (seen: HashSet<FSharpAssembly>) =
    let items = Dictionary<string, Item>(StringComparer.Ordinal)

    let add (item: Item) =
        if items.ContainsKey item.DocId then
            ()
        else
            items.[item.DocId] <- item

    let note (t: FSharpType) =
        if t.HasTypeDefinition then
            seen.Add t.TypeDefinition.Assembly |> ignore

    let addMember (owner: FSharpEntity) (parent: string) (m: ApiDocMember) =
        let docs = readDocs m.Comment

        let common (docId: string) (kind: string) (name: string) (display: string) (signature: string) (range: FSharp.Compiler.Text.range) =
            let file, line = locate range docId

            add
                { DocId = docId
                  Kind = kind
                  Name = name
                  DisplayName = display
                  Parent = Some parent
                  Signature = signature
                  Summary = docs.Summary
                  Remarks = docs.Remarks
                  Params = []
                  Returns = None
                  Examples = docs.Examples
                  File = file
                  Line = line
                  Implements = [] }

        match box m.Symbol with
        | :? FSharpMemberOrFunctionOrValue as v when not m.Exclude && memberVisible owner v ->
            let kind = memberKind owner v
            let docId = docIdOf v.XmlDocSig "M" v.FullName
            let file, line = locate v.DeclarationLocation docId
            note v.ReturnParameter.Type

            for group in v.CurriedParameterGroups do
                for p in group do
                    note p.Type

            let parameters = if kind = "field" || kind = "const" then [] else parametersOf v docs
            let returns = if kind = "constructor" || kind = "field" || kind = "const" then None else returnsOf v docs

            add
                { DocId = docId
                  Kind = kind
                  Name = memberName owner v
                  DisplayName = memberName owner v
                  Parent = Some parent
                  Signature = memberSignature owner v kind
                  Summary = docs.Summary
                  Remarks = docs.Remarks
                  Params = parameters
                  Returns = returns
                  Examples = docs.Examples
                  File = file
                  Line = line
                  Implements = [] }
        | :? FSharpUnionCase as c when not m.Exclude && c.Accessibility.IsPublic ->
            common (docIdOf c.XmlDocSig "U" c.FullName) "union_case" c.Name c.Name (caseSignature c) c.DeclarationLocation
        | :? FSharpField as f when not m.Exclude && fieldVisible owner f ->
            let kind =
                if owner.IsEnum then "enum_field"
                else "field"

            let signature = if owner.IsEnum then enumSignature f else fieldSignature f
            common (docIdOf f.XmlDocSig "F" (owner.DisplayName + "." + f.Name)) kind f.Name f.Name signature f.DeclarationLocation
        | _ -> ()

    let rec addEntity (parent: string) (e: ApiDocEntity) =
        if not e.Exclude then
            let symbol = box e.Symbol :?> FSharpEntity
            let docs = readDocs e.Comment
            let docId = entityDocId symbol
            let file, line = locate symbol.DeclarationLocation docId

            let implements =
                [ for i in symbol.DeclaredInterfaces do
                      if i.HasTypeDefinition then
                          yield entityDocId i.TypeDefinition ]

            (match symbol.BaseType with
             | Some b -> note b
             | None -> ())

            add
                { DocId = docId
                  Kind = entityKind symbol
                  Name = entityName symbol
                  DisplayName = symbol.DisplayName
                  Parent = Some parent
                  Signature = entitySignature symbol
                  Summary = docs.Summary
                  Remarks = docs.Remarks
                  Params = []
                  Returns = None
                  Examples = docs.Examples
                  File = file
                  Line = line
                  Implements = implements }

            let members =
                e.UnionCases
                @ e.RecordFields
                @ e.Constructors
                @ e.InstanceMembers
                @ e.StaticMembers
                @ e.ValuesAndFuncs
                @ e.TypeExtensions
                @ e.ActivePatterns

            for m in members do
                addMember symbol docId m

            for nested in e.NestedEntities do
                addEntity docId nested

    for ns in model.Collection.Namespaces do
        for e in ns.Entities do
            if not e.Exclude then
                let symbol = box e.Symbol :?> FSharpEntity
                let name = namespaceOf symbol
                let nsId = "N:" + name

                if not (items.ContainsKey nsId) then
                    items.[nsId] <- namespaceItem name

                addEntity nsId e

    items

let rec directIds (e: FSharpEntity) (ids: HashSet<string>) =
    if e.Accessibility.IsPublic then
        ids.Add(entityDocId e) |> ignore

        if not e.IsFSharpAbbreviation then
            for v in e.MembersFunctionsAndValues do
                if memberVisible e v then
                    ids.Add(docIdOf v.XmlDocSig "M" v.FullName) |> ignore

        for c in e.UnionCases do
            if c.Accessibility.IsPublic then
                ids.Add(docIdOf c.XmlDocSig "U" c.FullName) |> ignore

        for f in e.FSharpFields do
            if fieldVisible e f then
                ids.Add(docIdOf f.XmlDocSig "F" (e.DisplayName + "." + f.Name)) |> ignore

        for nested in e.NestedEntities do
            directIds nested ids

let crossCheck (assembly: FSharpAssembly) (items: Dictionary<string, Item>) =
    let expected = HashSet<string>(StringComparer.Ordinal)

    for e in assembly.Contents.Entities do
        if e.Accessibility.IsPublic then
            expected.Add("N:" + namespaceOf e) |> ignore
            directIds e expected

    let actual = HashSet<string>(items.Keys, StringComparer.Ordinal)
    let missing = expected |> Seq.filter (fun id -> not (actual.Contains id)) |> Seq.sort |> List.ofSeq
    let extra = actual |> Seq.filter (fun id -> not (expected.Contains id)) |> Seq.sort |> List.ofSeq

    if not missing.IsEmpty || not extra.IsEmpty then
        fail (
            sprintf
                "FSharp.Formatting and the compiled assembly disagree about the public API; missing from FSharp.Formatting: [%s]; not in the assembly: [%s]"
                (String.Join(", ", missing))
                (String.Join(", ", extra))
        )

let checkFramework (seen: HashSet<FSharpAssembly>) =
    let runtime = Path.GetDirectoryName(typeof<obj>.Assembly.Location)
    let root = Path.GetFullPath(Path.Combine(runtime, "..", "..", ".."))

    for assembly in seen do
        let name = assembly.SimpleName

        if frameworkPrefixes |> List.exists (fun p -> name.StartsWith(p, StringComparison.Ordinal)) then
            match assembly.FileName with
            | Some file when not (Path.GetFullPath(file).StartsWith(root, StringComparison.Ordinal)) ->
                fail (sprintf "the framework assembly '%s' resolved to '%s', outside the toolchain root '%s'" name file root)
            | _ -> ()

let render' (context: Context) (items: Item list) =
    use stream = new MemoryStream()

    let options =
        JsonWriterOptions(Indented = true, NewLine = "\n", Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping)

    use writer = new Utf8JsonWriter(stream, options)
    writer.WriteStartObject()
    writer.WriteString("schema", Schema)
    writer.WriteString("language", "fsharp")
    writer.WriteString("assembly", context.Assembly)
    writer.WriteStartArray "items"

    for item in items do
        writer.WriteStartObject()
        writer.WriteString("doc_id", item.DocId)
        writer.WriteString("kind", item.Kind)
        writer.WriteString("name", item.Name)
        writer.WriteString("display_name", item.DisplayName)

        match item.Parent with
        | Some parent -> writer.WriteString("parent_doc_id", parent)
        | None -> writer.WriteNull "parent_doc_id"

        writer.WriteString("signature", item.Signature)
        writer.WriteString("summary", item.Summary)
        writer.WriteString("remarks", item.Remarks)
        writer.WriteStartArray "params"

        for name, typeName, doc in item.Params do
            writer.WriteStartObject()
            writer.WriteString("name", name)
            writer.WriteString("type", typeName)
            writer.WriteString("doc", doc)
            writer.WriteEndObject()

        writer.WriteEndArray()

        match item.Returns with
        | Some(typeName, doc) ->
            writer.WriteStartObject "returns"
            writer.WriteString("type", typeName)
            writer.WriteString("doc", doc)
            writer.WriteEndObject()
        | None -> writer.WriteNull "returns"

        writer.WriteStartArray "examples"

        for example in item.Examples do
            writer.WriteStringValue example

        writer.WriteEndArray()
        writer.WriteString("file", item.File)
        writer.WriteNumber("line", item.Line)
        writer.WriteStartArray "implements"

        for target in item.Implements |> List.sortWith (fun a b -> String.CompareOrdinal(a, b)) |> List.distinct do
            writer.WriteStringValue target

        writer.WriteEndArray()
        writer.WriteEndObject()

    writer.WriteEndArray()
    writer.WriteEndObject()
    writer.Flush()
    Encoding.UTF8.GetString(stream.ToArray()) + "\n"

let run (context: Context) (scratch: string) =
    if context.Srcs.IsEmpty then
        fail "the library has no F# sources"

    let dll = Path.GetFullPath context.Dll
    let xml = Path.GetFullPath context.Xml
    let depRefs = context.DepRefs |> List.map Path.GetFullPath
    let root = Path.GetFullPath scratch
    let temp = Path.Combine(root, "tmp")
    let refs = Path.Combine(root, "refs")
    Directory.CreateDirectory temp |> ignore
    Directory.CreateDirectory refs |> ignore

    for name in [ "TMPDIR"; "TEMP"; "TMP" ] do
        Environment.SetEnvironmentVariable(name, temp)

    Environment.SetEnvironmentVariable("DOTNET_CLI_HOME", Path.Combine(root, "home"))
    Directory.SetCurrentDirectory root

    for file in depRefs do
        File.Copy(file, Path.Combine(refs, Path.GetFileName file), true)

    let errors = List<string>()
    let input = { ApiDocInput.FromFile(dll, publicOnly = true) with XmlFile = Some xml }

    let model =
        ApiDocs.GenerateModel([ input ], context.Assembly, [], libDirs = [ refs ], onError = (fun message -> errors.Add message))

    if errors.Count > 0 then
        fail ("FSharp.Formatting reported errors:\n" + String.Join("\n", errors))

    let seen = HashSet<FSharpAssembly>()
    let items = walkModel model seen

    let first =
        model.Collection.Namespaces
        |> Seq.collect (fun ns -> ns.Entities)
        |> Seq.tryHead

    match first with
    | None -> fail "the library has no public API; FSharp.Formatting found no items"
    | Some entity ->
        let symbol = box entity.Symbol :?> FSharpEntity
        seen.Add symbol.Assembly |> ignore
        checkFramework seen
        crossCheck symbol.Assembly items

    let ordered = items.Values |> Seq.sortWith (fun a b -> String.CompareOrdinal(a.DocId, b.DocId)) |> List.ofSeq
    render' context ordered

[<EntryPoint>]
let main argv =
    let mutable contextPath = ""
    let mutable outPath = ""
    let mutable scratch = ""
    let mutable usage = false
    let mutable index = 0

    while index + 1 < argv.Length do
        match argv.[index] with
        | "--context" -> contextPath <- argv.[index + 1]
        | "--out" -> outPath <- argv.[index + 1]
        | "--scratch" -> scratch <- argv.[index + 1]
        | other ->
            eprintfn "fsharp_api: unknown argument %s" other
            usage <- true

        index <- index + 2

    if usage then
        2
    elif contextPath = "" || outPath = "" || scratch = "" || index <> argv.Length then
        eprintfn "usage: fsharp_api --context FILE --scratch DIR --out FILE"
        2
    else
        try
            let context = readContext contextPath
            let outFull = Path.GetFullPath outPath
            let json = run context scratch
            File.WriteAllText(outFull, json, UTF8Encoding false)
            0
        with
        | Failure message ->
            eprintfn "fsharp_api: %s" message
            1
        | error ->
            eprintfn "fsharp_api: %O" error
            1

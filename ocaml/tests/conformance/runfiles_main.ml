let strip_cr line =
  let n = String.length line in
  if n > 0 && line.[n - 1] = '\r' then String.sub line 0 (n - 1) else line

let from_manifest manifest rpath =
  let prefix = rpath ^ " " in
  let plen = String.length prefix in
  let ic = open_in manifest in
  let rec scan () =
    match input_line ic with
    | line ->
        let line = strip_cr line in
        if String.length line >= plen && String.sub line 0 plen = prefix then
          String.sub line plen (String.length line - plen)
        else scan ()
    | exception End_of_file -> failwith ("runfile not in manifest: " ^ rpath)
  in
  Fun.protect ~finally:(fun () -> close_in ic) scan

let resolve rpath =
  match Sys.getenv_opt "RUNFILES_DIR" with
  | Some dir when dir <> "" -> Filename.concat dir rpath
  | _ -> (
      match Sys.getenv_opt "RUNFILES_MANIFEST_FILE" with
      | Some manifest when manifest <> "" -> from_manifest manifest rpath
      | _ -> failwith "no runfiles")

let () =
  let ic = open_in_bin (resolve Sys.argv.(1)) in
  let text = really_input_string ic (in_channel_length ic) in
  close_in ic;
  print_string text

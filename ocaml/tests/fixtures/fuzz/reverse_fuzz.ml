let reverse l = List.rev l

let () =
  Crowbar.add_test ~name:"reverse twice"
    Crowbar.[ list int ]
    (fun l -> Crowbar.check_eq (reverse (reverse l)) l)

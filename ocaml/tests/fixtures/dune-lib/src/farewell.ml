external stamp : unit -> int = "dx_farewell_stamp"

let bye name = Greet.hello name ^ " " ^ string_of_int (stamp ())

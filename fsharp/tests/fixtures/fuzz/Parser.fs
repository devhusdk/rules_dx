namespace Fixtures.Fuzz

module Parser =
    let parse (text: string) =
        if text.Length > 2 && text[0] = 'F' && text[1] = 'U' && text[2] = 'Z' then
            raise (System.InvalidOperationException "parser crash")

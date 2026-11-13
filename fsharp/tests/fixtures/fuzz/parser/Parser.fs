namespace Fixtures.Fuzz

module Parser =
    let parse (text: string) =
        if text.Length > 2 && text[0] = 'F' then
            raise (System.InvalidOperationException "parser crash")

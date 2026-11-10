namespace Fixtures.Fuzz

module Wrapper =
    let handle (text: string) = Parser.parse text

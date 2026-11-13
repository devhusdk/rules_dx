namespace Fixtures.Fuzz

open Fixtures.Fuzz.Parser

module Wrapper =
    let handle (text: string) = parse text

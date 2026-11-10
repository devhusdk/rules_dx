// F# compile-order fixture library; depends on Helper.fs.
module Demo

let greet name = "hello " + name + Helper.suffix

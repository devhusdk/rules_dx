@_cdecl("LLVMFuzzerTestOneInput")
public func fuzzOne(_ data: UnsafePointer<UInt8>, _ size: Int) -> CInt {
    let bytes = UnsafeBufferPointer(start: data, count: size)
    if size >= 3 && bytes[0] == UInt8(ascii: "f") && bytes[1] == UInt8(ascii: "u") && bytes[2] == UInt8(ascii: "z") {
        return CInt(bytes[size])
    }
    return 0
}

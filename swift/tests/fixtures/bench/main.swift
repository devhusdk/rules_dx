import DxBench

var total = 0
dxBench("parse_small", iterations: 100) {
    total &+= "12345".utf8.reduce(0) { $0 &* 10 &+ Int($1 &- 48) }
}
if total == 0 { print("unreachable") }

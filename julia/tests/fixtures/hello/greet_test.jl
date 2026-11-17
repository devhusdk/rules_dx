using Greet

greeting("julia") == "Hello, julia!" || error("greeting: got $(repr(greeting("julia")))")

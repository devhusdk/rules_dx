using Greet: greeting

greeting("julia") == "Hello, julia!" || error("greeting: got $(repr(greeting("julia")))")

package app

import "example.com/app/lib"

greeting: lib.#Greeting & {
	name: "world"
}

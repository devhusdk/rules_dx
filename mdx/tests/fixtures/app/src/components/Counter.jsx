import { useState } from "preact/hooks";

export default function Counter({ start }) {
	const [count, setCount] = useState(start);
	return (
		<button type="button" class="counter" onClick={() => setCount(count + 1)}>
			Count: {count}
		</button>
	);
}

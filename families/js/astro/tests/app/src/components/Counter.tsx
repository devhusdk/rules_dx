import { useState } from "preact/hooks";

interface Props {
	start: number;
}

export default function Counter({ start }: Props) {
	const [count, setCount] = useState(start);
	return (
		<button type="button" class="counter" onClick={() => setCount(count + 1)}>
			Count: {count}
		</button>
	);
}

import { createClient, OPERATIONS } from "./client.js";
import type { Ops } from "./client.js";

// @ts-expect-error node:test ships no local types here; node provides it at runtime.
import { test } from "node:test";

function equal(actual: unknown, expected: unknown): void {
	if (actual !== expected) {
		throw new Error(`want ${String(expected)} got ${String(actual)}`);
	}
}

test("getOrder decodes the fixture response", async () => {
	const requested: string[] = [];
	const fixture = (async (input: unknown): Promise<Response> => {
		requested.push(String(input));
		return new Response(JSON.stringify({ id: "7", total: 12 }), {
			status: 200,
			headers: { "content-type": "application/json" },
		});
	}) as typeof fetch;
	const api = createClient<Ops>(OPERATIONS, { serverUrl: "http://shop.test", fetch: fixture });
	const order = await api.getOrder({ path: { orderId: "7" } });
	equal(requested.length, 1);
	equal(requested[0], "http://shop.test/orders/7");
	equal(order.id, "7");
	equal(order.total, 12);
});

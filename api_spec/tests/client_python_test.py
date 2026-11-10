import os
import sys
import unittest

import httpx

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from petstore_py.api.default import get_pet
from petstore_py.client import Client


def handler(request: httpx.Request) -> httpx.Response:
    if request.method == "GET" and request.url.path == "/pets/7":
        return httpx.Response(200, json={"id": 7, "name": "Rex", "tag": "dog"})
    return httpx.Response(404)


class ClientPythonTest(unittest.TestCase):
    def test_get_pet_decodes_fixture_response(self):
        client = Client(base_url="http://petstore.test")
        client.set_httpx_client(
            httpx.Client(base_url="http://petstore.test", transport=httpx.MockTransport(handler))
        )
        pet = get_pet.sync(pet_id="7", client=client)
        self.assertEqual(pet.id, 7)
        self.assertEqual(pet.name, "Rex")
        self.assertEqual(pet.tag, "dog")


if __name__ == "__main__":
    unittest.main()

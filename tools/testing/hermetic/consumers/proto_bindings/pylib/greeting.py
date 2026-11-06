from api.v1 import greeting_pb2


def round_trip(name, times):
    data = greeting_pb2.Greeting(name=name, times=times).SerializeToString()
    return greeting_pb2.Greeting.FromString(data)

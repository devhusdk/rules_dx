from api.v1 import greeting_pb2


def main():
    data = greeting_pb2.Greeting(name="py", times=2).SerializeToString()
    back = greeting_pb2.Greeting.FromString(data)
    print(back.name, back.times)


if __name__ == "__main__":
    main()

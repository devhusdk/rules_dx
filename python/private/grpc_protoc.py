import sys

from grpc_tools import protoc

sys.exit(protoc.main(["protoc"] + sys.argv[1:]))

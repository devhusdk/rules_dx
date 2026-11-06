using System;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Xunit;

namespace ProtoFixture;

public class ProtoTests
{
    [Fact]
    public void OrderSurvivesRoundTrip()
    {
        var order = new Order
        {
            Item = new Item { Name = "pen" },
            Placed = Timestamp.FromDateTimeOffset(DateTimeOffset.FromUnixTimeSeconds(42)),
        };

        var parsed = Order.Parser.ParseFrom(order.ToByteArray());

        Assert.Equal("pen", parsed.Item.Name);
        Assert.Equal(42, parsed.Placed.Seconds);
    }

    [Fact]
    public void OddlyNamedFilesGenerate()
    {
        var user = UserV1Msg.Parser.ParseFrom(new UserV1Msg { Id = "u" }.ToByteArray());
        var code = Code.Parser.ParseFrom(new Code { Id = "c" }.ToByteArray());

        Assert.Equal("u", user.Id);
        Assert.Equal("c", code.Id);
    }
}

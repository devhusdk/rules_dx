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
}

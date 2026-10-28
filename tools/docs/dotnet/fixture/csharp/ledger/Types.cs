namespace Dx.Fixture.Ledger
{
    /// <summary>Something that can be audited.</summary>
    public interface IAuditable
    {
        /// <summary>Audits the instance.</summary>
        /// <returns>The audit result.</returns>
        string Audit();
    }

    /// <summary>A pair of cents and a label.</summary>
    public struct Entry
    {
        /// <summary>The amount in cents.</summary>
        public long Cents;

        /// <summary>The label of the entry.</summary>
        public string Label;
    }

    /// <summary>The kind of an account.</summary>
    public enum AccountKind
    {
        /// <summary>A checking account.</summary>
        Checking,

        /// <summary>A savings account.</summary>
        Savings,
    }

    internal class Hidden
    {
        public void Visible()
        {
        }
    }

    /// <summary>A class with an implicit constructor.</summary>
    public class Plain
    {
        /// <summary>A public field.</summary>
        public int X;
    }

    /// <summary>A positional record.</summary>
    /// <param name="A">The first component.</param>
    public record Point(int A);
}

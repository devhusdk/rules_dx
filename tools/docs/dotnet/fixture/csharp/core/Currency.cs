namespace Dx.Fixture.Core
{
    /// <summary>A currency code used by the ledger.</summary>
    public sealed class Currency
    {
        /// <summary>Creates a currency.</summary>
        /// <param name="code">The three letter code.</param>
        public Currency(string code)
        {
            Code = code;
        }

        /// <summary>Gets the three letter code.</summary>
        public string Code { get; }
    }
}

#nullable enable

namespace Dx.Fixture.Ledger
{
    /// <summary>Looks up an account name.</summary>
    public static class Names
    {
        /// <summary>Finds a name.</summary>
        /// <param name="key">The lookup key.</param>
        /// <returns>The name, or null.</returns>
        public static string? Find(string key)
        {
            return key.Length == 0 ? null : key;
        }
    }
}

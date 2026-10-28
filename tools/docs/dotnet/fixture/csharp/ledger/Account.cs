using System;
using Dx.Fixture.Core;

namespace Dx.Fixture.Ledger
{
    /// <summary>A ledger account that holds a balance.</summary>
    /// <remarks>Accounts are not thread safe.</remarks>
    public class Account : IAuditable
    {
        private readonly Currency currency;

        /// <summary>Creates an empty account.</summary>
        /// <param name="currency">The currency of the account.</param>
        public Account(Currency currency)
        {
            this.currency = currency;
        }

        /// <summary>Creates an account with an opening balance.</summary>
        /// <param name="currency">The currency of the account.</param>
        /// <param name="opening">The opening balance.</param>
        public Account(Currency currency, decimal opening)
        {
            this.currency = currency;
            Balance = opening;
        }

        /// <summary>Raised when the balance changes.</summary>
        public event EventHandler Changed;

        /// <summary>Gets the current balance.</summary>
        public decimal Balance { get; private set; }

        /// <summary>Adds an amount to the balance.</summary>
        /// <param name="amount">The amount to add.</param>
        /// <returns>The new balance.</returns>
        public decimal Deposit(decimal amount)
        {
            Balance += amount;
            Changed?.Invoke(this, EventArgs.Empty);
            return Balance;
        }

        /// <summary>Adds a whole number of cents to the balance.</summary>
        /// <param name="cents">The cents to add.</param>
        /// <returns>The new balance.</returns>
        public decimal Deposit(long cents)
        {
            return Deposit(cents / 100m);
        }

        /// <summary>Gets the currency of the account.</summary>
        /// <returns>The currency.</returns>
        public Currency GetCurrency()
        {
            return currency;
        }

        /// <summary>Returns the larger of two values.</summary>
        /// <typeparam name="T">The compared type.</typeparam>
        /// <param name="left">The first value.</param>
        /// <param name="right">The second value.</param>
        /// <returns>The larger value.</returns>
        public static T Larger<T>(T left, T right) where T : IComparable<T>
        {
            return left.CompareTo(right) >= 0 ? left : right;
        }

        /// <inheritdoc />
        public string Audit()
        {
            return "ok";
        }

#if DX_EXTRA
        /// <summary>Freezes the account; only built with DX_EXTRA.</summary>
        public void Freeze()
        {
        }
#endif
    }
}

namespace Dx.Fixture.FsLedger

open Dx.Fixture.Core

/// A whole amount of cents.
type Cents = int64

/// The kind of an account.
type AccountKind =
    /// Day to day use.
    | Checking = 0
    /// Interest bearing.
    | Savings = 1

/// A ledger entry.
type Entry =
    {
        /// The amount in cents.
        Cents: int64
        /// The free text memo.
        Memo: string
    }

/// The outcome of a transaction.
type Outcome =
    /// The transaction went through.
    | Posted of id: int
    /// The transaction was refused.
    | Refused of reason: string * code: int
    /// Nothing happened.
    | Skipped

/// Something that can be audited.
type IAuditable =
    /// Audits the instance.
    abstract Audit: unit -> string

type internal Hidden = { Secret: int }

/// <summary>A ledger account that holds a balance.</summary>
/// <remarks>Accounts are not thread safe.</remarks>
type Account(currency: Currency) =
    let mutable balance = 0m

    /// Gets the currency of the account.
    member _.Currency = currency

    /// Gets or sets the balance.
    member _.Balance
        with get () = balance
        and set (value: decimal) = balance <- value

    /// Adds a whole number of cents to the balance.
    member _.Deposit(cents: int64) : decimal =
        balance <- balance + decimal cents / 100m
        balance

    /// Adds an amount to the balance.
    member _.Deposit(amount: decimal) : decimal =
        balance <- balance + amount
        balance

    interface IAuditable with
        member _.Audit() = "audited"

/// Functions that work on accounts.
module Account =
    /// Creates an account in a currency.
    let create (currency: Currency) = Account(currency)

/// Helpers for amounts.
module Amounts =
    /// Multiplies an amount by a factor.
    let scale (factor: int64) (amount: int64) = factor * amount

    /// <summary>Adds two amounts.</summary>
    /// <param name="left">The left amount.</param>
    /// <param name="right">The right amount, see <see cref="T:Dx.Fixture.FsLedger.Entry"/>.</param>
    /// <returns>The sum of both amounts.</returns>
    let add (left: int64) (right: int64) : int64 = left + right

    let undocumented (value: int) = value + 1

    let private hidden (value: int) = value * 2

    let internal visibleInside (value: int) = hidden value

#if DX_EXTRA
    /// Only present when DX_EXTRA is defined.
    let extra (value: int) = value
#endif

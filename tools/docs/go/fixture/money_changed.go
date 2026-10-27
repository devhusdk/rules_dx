// Package money keeps ledgers of accounts.
//
// # Accounts
//
// Open an account with [NewAccount].
package money

import "errors"

// Amount is a sum of money in cents.
type Amount int64

// Currency names a currency.
type Currency int

// The supported currencies.
const (
	EUR Currency = iota
	USD
)

// MaxAmount caps every balance.
const MaxAmount Amount = 1_000_000_00

// DefaultCurrency is used when none is given.
var DefaultCurrency = EUR

// ErrOverdrawn reports a withdrawal past zero.
var ErrOverdrawn = errors.New("overdrawn")

// Cents is another name for Amount.
type Cents = Amount

// Account holds the balance of one owner.
type Account struct {
	// Owner names the holder.
	Owner   string
	balance Amount
}

// NewAccount opens an empty account.
func NewAccount(owner string) *Account {
	return &Account{Owner: owner}
}

// Deposit adds an amount to the balance and never fails.
func (a *Account) Deposit(amount Amount) {
	a.balance += amount
}

func (a *Account) audit() bool {
	return a.balance >= 0
}

// Ledger stores accounts.
type Ledger interface {
	// Find returns the account of one owner.
	Find(owner string) (*Account, error)
}

// Pair holds two values.
type Pair[K comparable, V any] struct {
	// Key is the first value.
	Key K
}

// Sum adds amounts.
func Sum[T ~int64](values ...T) T {
	var total T
	for _, v := range values {
		total += v
	}
	return total
}

func round(a Amount) Amount {
	return a
}

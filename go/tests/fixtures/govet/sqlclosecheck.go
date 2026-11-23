package govet

import (
	"context"
	"database/sql"
)

func First(ctx context.Context, db *sql.DB) (string, error) {
	rows, err := db.QueryContext(ctx, "SELECT name FROM t")
	if err != nil {
		return "", err
	}
	var name string
	if rows.Next() {
		err = rows.Scan(&name)
	}
	return name, err
}

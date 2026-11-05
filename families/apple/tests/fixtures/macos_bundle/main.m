long long ledger_total(const char *text);

int main(void) {
    return ledger_total("rent 1000\n") == 1000 ? 0 : 1;
}

int sign(int value);

int sign(int value) {
  if (value < 0) {
    return -1;
  } else {
    return 1;
  }
}

int main(void) { return sign(1); }

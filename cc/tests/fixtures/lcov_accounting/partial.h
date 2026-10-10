// Partial-coverage fixture header; declarations only, no executable lines.
#pragma once

// PartialAdd returns the sum of a and b; exercised by the fixture test.
int PartialAdd(int a, int b);

// PartialClamp maps negatives to 0; both branches exercised by the test.
int PartialClamp(int value);

// PartialUntested is never called by the fixture test; its lines stay cold.
int PartialUntested(int value);

#ifndef EXAMPLES_CPP_APP_CORE_GROUP_H_
#define EXAMPLES_CPP_APP_CORE_GROUP_H_

#include <map>
#include <vector>

namespace pantry {

template <typename Row, typename KeyFn, typename ValueFn>
auto group_sum(const std::vector<Row> &rows, KeyFn key, ValueFn value) {
  using Key = std::decay_t<decltype(key(rows.front()))>;
  using Value = std::decay_t<decltype(value(rows.front()))>;
  std::map<Key, Value> totals;
  for (const Row &row : rows) {
    totals[key(row)] += value(row);
  }
  return totals;
}

} // namespace pantry

#endif

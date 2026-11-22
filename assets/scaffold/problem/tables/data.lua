---@diagnostic disable: undefined-global
-- 写死的静态表格
local il = tng.tools.inline_latex

return tng.table {
  headers = { "子任务", "分值", il("n"), il("m") },
  align = { "center", "center", "center", "center" },
  data = {
    { "1", "30", il("\\le 10^{3}"), il("\\le 10^{3}") },
    { "2", "30", il("\\le 10^{5}"), il("\\le 10^{3}") },
    { "3", "40", il("\\le 10^{5}"), il("\\le 10^{9}") },
  },
  merge_rules = { { col = { 3, 4 }, merge_row = true } }
}

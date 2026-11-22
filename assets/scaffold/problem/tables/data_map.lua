---@diagnostic disable: undefined-global
-- 用 map 从 data_cases 生成表格
local il = tng.tools.inline_latex
local hn = tng.tools.hn
local cases = tng.tools.cases

return tng.table {
  headers = { "测试点", "分值", il("n"), il("m") },
  align = { "center", "center", "center", "center" },
  data = tng.config.data_cases:map(function(case)
    local args = case.args

    local count = 1
    if type(case.id) == "table" then
      count = #case.id
    end

    return {
      cases(case.id),
      tostring(case.score * count),
      il("\\le " .. hn(args.n)),
      il("\\le " .. hn(args.m)),
    }
  end),
  merge_rules = { { col = { 3, 4 }, merge_row = true } }
}

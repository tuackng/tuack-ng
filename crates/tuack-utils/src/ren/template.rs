use crate::prelude::*;
use crate::ren::tools;
use crate::utils::resolve_within;
use anyhow::Result;
use minijinja::value::Serde;
use minijinja::{Environment, Value, context};
use owo_colors::OwoColorize;
use std::sync::{Arc, Mutex};
use tuack_lib::ren::RenParams;

fn input_file(problem: &ProblemConfig, file_io: bool) -> Result<String, minijinja::Error> {
    Ok(if file_io {
        format!("从文件 _{}.in_ 中读入数据。", problem.name)
    } else {
        "从标准输入读入数据。".to_string()
    })
}

fn output_file(problem: &ProblemConfig, file_io: bool) -> Result<String, minijinja::Error> {
    Ok(if file_io {
        format!("输出到文件 _{}.out_ 中。", problem.name)
    } else {
        "输出到标准输出。".to_string()
    })
}

/// 实现模板函数 `sample.text(id)`：把 `base_path/sample/` 下的样例文件渲染为
/// Markdown 小节；样例配置缺失或文件读取失败时记录警告并返回错误，样例文件不存在时只返回错误
fn handle_sample(
    sample_id: u32,
    problem: &ProblemConfig,
    base_path: &Path,
    warnings: &Arc<Mutex<Vec<String>>>,
) -> Result<String, minijinja::Error> {
    debug!("处理 sample 函数 {}", sample_id);

    // 查找样例
    let sample_item = match problem.samples.iter().find(|s| s.id == sample_id) {
        Some(item) => item,
        None => {
            warnings.lock().unwrap().push(format!(
                "在题目 {} 中未找到样例 {}",
                problem.name.magenta(),
                sample_id.to_string().cyan()
            ));
            return Err(minijinja::Error::new(
                minijinja::ErrorKind::InvalidOperation,
                format!("未找到样例 {}", sample_id),
            ));
        }
    };

    // 构建 Markdown
    let mut md = String::new();

    // 输入部分
    md.push_str(&format!("## 样例 {} 输入\n\n", sample_id));

    let input_file = &sample_item.input_path();

    let input_path =
        resolve_within(base_path, &Path::new("sample").join(input_file)).map_err(|error| {
            minijinja::Error::new(
                minijinja::ErrorKind::InvalidOperation,
                format!("无法解析样例 {sample_id} 的输入文件：{error:#}"),
            )
        })?;
    if input_path.exists() {
        match fs::read_to_string(&input_path) {
            Ok(content) => {
                md.push_str("```txt\n");
                md.push_str(&content);
                if !content.ends_with('\n') {
                    md.push('\n');
                }
                md.push_str("```\n\n");
            }
            Err(e) => {
                warnings
                    .lock()
                    .unwrap()
                    .push(format!("读取输入文件 {:?} 失败：{}", input_path, e));
                return Err(minijinja::Error::new(
                    minijinja::ErrorKind::InvalidOperation,
                    format!("读取输入文件失败 {}", e),
                ));
            }
        }
    } else {
        return Err(minijinja::Error::new(
            minijinja::ErrorKind::InvalidOperation,
            format!("文件不存在 {}", input_path.display()),
        ));
    }

    // 输出部分
    md.push_str(&format!("## 样例 {} 输出\n\n", sample_id));

    let output_file = &sample_item.output_path();

    let output_path =
        resolve_within(base_path, &Path::new("sample").join(output_file)).map_err(|error| {
            minijinja::Error::new(
                minijinja::ErrorKind::InvalidOperation,
                format!("无法解析样例 {sample_id} 的输出文件：{error:#}"),
            )
        })?;
    if output_path.exists() {
        match fs::read_to_string(&output_path) {
            Ok(content) => {
                md.push_str("```txt\n");
                md.push_str(&content);
                if !content.ends_with('\n') {
                    md.push('\n');
                }
                md.push_str("```\n");
            }
            Err(e) => {
                warnings
                    .lock()
                    .unwrap()
                    .push(format!("读取输出文件失败：{:?} -> {}", output_path, e));
                return Err(minijinja::Error::new(
                    minijinja::ErrorKind::InvalidOperation,
                    format!("读取输出文件失败 {}", e),
                ));
            }
        }
    } else {
        return Err(minijinja::Error::new(
            minijinja::ErrorKind::InvalidOperation,
            format!("输出文件不存在 {}", output_path.display()),
        ));
    }

    debug!("成功生成样例 {} 的 Markdown", sample_id);
    Ok(md)
}

/// 实现模板函数 `sample.file(id)`：生成引用选手目录样例文件的语句；样例缺失时仅记录警告
fn handle_sample_file(
    sample_id: u32,
    problem: &ProblemConfig,
    warnings: &Arc<Mutex<Vec<String>>>,
) -> Result<String, minijinja::Error> {
    debug!("处理 sample_file 函数 {}", sample_id);

    // 检查样例是否存在
    if !problem.samples.iter().any(|s| s.id == sample_id) {
        warnings.lock().unwrap().push(format!(
            "在题目 {} 中未找到样例 {}",
            problem.name.magenta(),
            sample_id.to_string().cyan()
        ));
    }

    let text = format!(
        "见选手目录下的 _{0}/{0}{1}.in_ 与 _{0}/{0}{1}.ans_。",
        problem.name, sample_id
    );

    debug!("生成文件引用：sample_file({}) -> {}", sample_id, text);
    Ok(text)
}

fn handle_lua_table(
    path: String,
    problem: &ProblemConfig,
    day: &ContestDayConfig,
    contest: &ContestConfig,
) -> Result<String, minijinja::Error> {
    let full_path = resolve_within(&problem.path, Path::new(&path)).map_err(|error| {
        minijinja::Error::new(
            minijinja::ErrorKind::InvalidOperation,
            format!("无法解析 Lua 表格 {path}：{error:#}"),
        )
    })?;
    super::lua::render_template(&full_path, problem, day, contest).map_err(|e| {
        minijinja::Error::new(
            minijinja::ErrorKind::InvalidOperation,
            format!("Lua 表格渲染失败：{}", e),
        )
    })
}

/// 模板 `cases` 的取值：接受单个整数或整数序列，返回区间表示的公式
fn cases_value(value: Value) -> Result<String, minijinja::Error> {
    let cases_vec: Vec<i32> = if let Some(i) = value.as_i64() {
        vec![i as i32]
    } else {
        value
            .try_iter()?
            .map(|v| {
                v.as_i64().map(|i| i as i32).ok_or_else(|| {
                    minijinja::Error::new(
                        minijinja::ErrorKind::InvalidOperation,
                        "cases 需要整数或整数序列",
                    )
                })
            })
            .collect::<Result<Vec<i32>, _>>()?
    };
    Ok(tools::cases(&cases_vec))
}

/// 模板 `int_lg`：以整数字符串输出，避免浮点表示
fn int_lg(num: f64) -> String {
    tools::int_lg(num).to_string()
}

/// 构建模板环境与上下文，供 [`render_template`] 渲染
///
/// # 上下文
///
/// `problem`、`day`、`contest`（依次为 [`ProblemConfig`]、[`ContestDayConfig`]、
/// [`ContestConfig`] 的全量视图），`data_cases`、`sample_cases`（测试点与样例列表）。
/// 函数命名空间：
/// - `sample.text(id)` / `sample.file(id)`：样例小节与样例文件引用
/// - `tools.int_lg` / `tools.hn(num, style)` / `tools.comma(num)` / `tools.spaces(num)` /
///   `tools.sci(num)` / `tools.cases(items)`：数字格式化
/// - `statement.input_file()` / `statement.output_file()`：输入输出方式语句
/// - `statement.table(path)`：渲染题目目录下 `path` 处的 Lua 表格
///
/// `tools` 下的函数同时注册为 `tools.` 前缀的过滤器，可用管道写法：`{{ num|tools.comma }}`、
/// `{{ num|tools.spaces }}`、`{{ num|tools.sci }}`、`{{ num|tools.hn("x") }}`、
/// `{{ items|tools.cases }}`。
///
/// `s` 为 `statement` 的别名；题目级 `file_io` 优先于 [`RenParams`] 中的 `file_io`。
///
/// `sample.*` 在渲染期间产生的警告写入 `warnings`；返回的环境绑定到 `'a`，只能渲染生命周期
/// 不短于 `'a` 的模板。
pub fn build_environment<'a>(
    problem: &ProblemConfig,
    day: &ContestDayConfig,
    contest: &ContestConfig,
    base_path: PathBuf,
    params: RenParams,
    warnings: &Arc<Mutex<Vec<String>>>,
) -> (Environment<'a>, Value) {
    // 创建环境
    let mut env = Environment::new();
    env.add_filter("tools.int_lg", int_lg);
    env.add_filter("tools.hn", tools::hn);
    env.add_filter("tools.comma", tools::comma);
    env.add_filter("tools.spaces", tools::spaces);
    env.add_filter("tools.sci", tools::sci);
    env.add_filter("tools.cases", cases_value);

    let sample = IndexMap::from([
        (
            "text",
            Value::from_function({
                let problem = problem.clone();
                let base_path = base_path.clone();
                let warnings = Arc::clone(warnings);
                move |sample_id: u32| -> Result<String, minijinja::Error> {
                    handle_sample(sample_id, &problem, &base_path, &warnings)
                }
            }),
        ),
        (
            "file",
            Value::from_function({
                let problem = problem.clone();
                let warnings = Arc::clone(warnings);
                move |sample_id: u32| -> Result<String, minijinja::Error> {
                    handle_sample_file(sample_id, &problem, &warnings)
                }
            }),
        ),
    ]);

    let tools = IndexMap::from([
        ("int_lg", Value::from_function(int_lg)),
        ("hn", Value::from_function(tools::hn)),
        ("comma", Value::from_function(tools::comma)),
        ("spaces", Value::from_function(tools::spaces)),
        ("sci", Value::from_function(tools::sci)),
        ("cases", Value::from_function(cases_value)),
    ]);

    let statement = IndexMap::from([
        (
            "input_file",
            Value::from_function({
                let problem = problem.clone();
                move || -> Result<String, minijinja::Error> {
                    input_file(&problem, problem.file_io.unwrap_or(params.file_io))
                }
            }),
        ),
        (
            "output_file",
            Value::from_function({
                let problem = problem.clone();
                move || -> Result<String, minijinja::Error> {
                    output_file(&problem, problem.file_io.unwrap_or(params.file_io))
                }
            }),
        ),
        (
            "table",
            Value::from_function({
                let problem = problem.clone();
                let day = day.clone();
                let contest = contest.clone();
                move |path: String| -> Result<String, minijinja::Error> {
                    handle_lua_table(path, &problem, &day, &contest)
                }
            }),
        ),
    ]);

    // 创建上下文
    let sample = Value::from_pairs(sample);
    let tools = Value::from_pairs(tools);
    let statement = Value::from_pairs(statement);
    let ctx = context! {
        problem => Serde(AsSerde::<ProblemConfig, FullView>::new(problem.clone())),
        day => Serde(AsSerde::<ContestDayConfig, FullView>::new(day.clone())),
        contest => Serde(AsSerde::<ContestConfig, FullView>::new(contest.clone())),
        data_cases => Serde(&problem.runtime.inherited_data),
        sample_cases => Serde(&problem.runtime.samples),

        sample => sample,
        tools => tools,
        statement => statement.clone(),
        s => statement
    };

    (env, ctx)
}

/// 渲染 MiniJinja 模板源码
///
/// 返回渲染结果与渲染过程中的警告（已带颜色）。模板可用的上下文变量与函数命名空间见
/// [`build_environment`]。
///
/// # Errors
///
/// 模板渲染失败时返回 `Err`：模板语法错误、`sample.text(id)` 的样例配置或样例文件缺失、
/// `statement.table(path)` 的 Lua 表格渲染失败，或 `tools.cases`（函数与过滤器）收到非整数项。
pub fn render_template(
    template: &str,
    problem: &ProblemConfig,
    day: &ContestDayConfig,
    contest: &ContestConfig,
    base_path: PathBuf,
    params: RenParams,
) -> Result<(String, Vec<String>)> {
    let warnings: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let (env, ctx) = build_environment(problem, day, contest, base_path, params, &warnings);

    let result = env.render_str(template, ctx)?;
    let warnings = warnings.lock().unwrap().clone();
    Ok((result, warnings))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn problem() -> ProblemConfig {
        serde_json::from_value::<AsSerde<ProblemConfig, FileView>>(serde_json::json!({
            "version": 7,
            "folder": "problem",
            "type": "program",
            "name": "p1",
            "title": "示例题目",
            "time limit": 1.0,
            "memory limit": "256 MiB",
            "dmk": "skip",
            "samples": [],
            "data": [],
            "subtasks": {}
        }))
        .expect("题目配置应可反序列化")
        .into_inner()
    }

    fn day() -> ContestDayConfig {
        serde_json::from_value::<AsSerde<ContestDayConfig, FileView>>(serde_json::json!({
            "version": 7,
            "folder": "day",
            "name": "day1",
            "subdir": [],
            "title": "Day 1",
            "compile": {},
            "start time": [2026, 1, 1, 8, 0, 0],
            "end time": [2026, 1, 1, 13, 0, 0]
        }))
        .expect("比赛日配置应可反序列化")
        .into_inner()
    }

    fn contest() -> ContestConfig {
        serde_json::from_value::<AsSerde<ContestConfig, FileView>>(serde_json::json!({
            "version": 7,
            "folder": "contest",
            "name": "demo",
            "subdir": [],
            "title": "Demo",
            "short title": "Demo"
        }))
        .expect("比赛配置应可反序列化")
        .into_inner()
    }

    fn env_and_ctx() -> (Environment<'static>, Value) {
        let warnings = Arc::new(Mutex::new(Vec::new()));
        build_environment(
            &problem(),
            &day(),
            &contest(),
            PathBuf::from("."),
            RenParams {
                use_pretest: false,
                noi_style: false,
                file_io: false,
            },
            &warnings,
        )
    }

    #[test]
    fn tools_filters_match_functions() {
        let (env, ctx) = env_and_ctx();

        for (filtered, called) in [
            ("{{ 1234|tools.comma }}", "{{ tools.comma(1234) }}"),
            ("{{ 1234|tools.spaces }}", "{{ tools.spaces(1234) }}"),
            ("{{ 1234|tools.sci }}", "{{ tools.sci(1234) }}"),
            ("{{ 2|tools.int_lg }}", "{{ tools.int_lg(2) }}"),
            (
                "{{ 1000000|tools.hn(\"x\") }}",
                "{{ tools.hn(1000000, \"x\") }}",
            ),
            ("{{ 1000000|tools.hn }}", "{{ tools.hn(1000000) }}"),
            (
                "{{ [1, 2, 3]|tools.cases }}",
                "{{ tools.cases([1, 2, 3]) }}",
            ),
            ("{{ 3|tools.cases }}", "{{ tools.cases(3) }}"),
        ] {
            assert_eq!(
                env.render_str(filtered, ctx.clone()).unwrap(),
                env.render_str(called, ctx.clone()).unwrap(),
                "{filtered}"
            );
        }

        assert_eq!(
            env.render_str("{{ 1000|tools.int_lg }}", ctx.clone())
                .unwrap(),
            "3"
        );
        assert_eq!(
            env.render_str("{{ 1234|tools.comma }}", ctx.clone())
                .unwrap(),
            "1{,}234"
        );
        assert_eq!(
            env.render_str("{{ 1234|tools.spaces }}", ctx.clone())
                .unwrap(),
            "1\\,234"
        );
        assert_eq!(
            env.render_str("{{ 1234|tools.sci }}", ctx).unwrap(),
            "1.234 \\times 10^{3}"
        );
    }
}

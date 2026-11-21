use std::time::Duration;

use bytesize::ByteSize;
use cel_interpreter::{Context as CelContext, Program, Value};
use clap::Args;
use csv::Writer;
use indicatif::ProgressBar;
use owo_colors::OwoColorize;

pub mod policy;

use crate::prelude::*;
use crate::test::policy::{DataPolicy, SamplePolicy, ScorePolicy as _};
use crate::utils::duration::format_duration;
use crate::utils::problem::{
    InteractivePurpose, build_runner, io_mode, problem_data_items, read_deps, resolve_problem_file,
    setup_interactive,
};
use crate::utils::progress::with_spinner;
use crate::utils::status::StatusLabel;
use crate::utils::target::Target;
use tuack_lib::test::{TaskParams, TestCaseStatus, TestSession};
use tuack_lib::utils::testlib::Checker;
use tuack_utils::checkers::{cpp::CppChecker, prebuilt::PrebuiltChecker};
use tuack_utils::data::FsTestData;

/// 展示状态：由 [`TestCaseStatus`] 映射而来，另加编译失败（CE）
#[derive(Debug, Clone)]
#[allow(clippy::upper_case_acronyms)]
pub(crate) enum DisplayStatus {
    AC,
    WA,
    RE,
    TLE,
    MLE,
    UKE,
    /// 文件错误：输出文件不存在
    FE,
    /// 部分正确，携带 0-100 的比例分
    PC(f64),
    CE,
}

impl From<&TestCaseStatus> for DisplayStatus {
    fn from(status: &TestCaseStatus) -> Self {
        match status {
            TestCaseStatus::AC => DisplayStatus::AC,
            TestCaseStatus::WA => DisplayStatus::WA,
            TestCaseStatus::RE => DisplayStatus::RE,
            TestCaseStatus::TLE => DisplayStatus::TLE,
            TestCaseStatus::MLE => DisplayStatus::MLE,
            TestCaseStatus::UKE => DisplayStatus::UKE,
            TestCaseStatus::FE => DisplayStatus::FE,
            TestCaseStatus::PC(p) => DisplayStatus::PC(*p),
        }
    }
}

/// 结果里的内存显示：MLE 时子进程在越限瞬间被终止，采到的峰值只是下界，用 `>` 标出
fn format_memory(status: &DisplayStatus, memory: Option<ByteSize>) -> String {
    match memory {
        Some(memory) if matches!(status, DisplayStatus::MLE) => format!("> {memory}"),
        Some(memory) => format!("{memory}"),
        None => "N/A".to_string(),
    }
}

/// 结果里的用时显示：TLE 时子进程由我们终止，实测时刻只是下界，用 `>` 标出
fn format_time(status: &DisplayStatus, time: Option<Duration>) -> String {
    match time {
        Some(time) if matches!(status, DisplayStatus::TLE) => {
            format!("> {}", format_duration(time))
        }
        Some(time) => format_duration(time),
        None => "N/A".to_string(),
    }
}

/// 单个测试点的评测结果
#[derive(Debug)]
pub struct IndividualTestCaseResult {
    pub test_case_id: u32,
    pub status: DisplayStatus,
    pub score: u32,
    pub full_score: u32,
    pub time: String,
    pub memory: String,
    /// 校验器诊断信息（完整内容，用于 CSV）
    pub message: Option<String>,
}

/// 单个测试者（题解）在其全部测试点上的汇总结果
#[derive(Debug)]
pub struct ProblemTestResult {
    pub tester_name: String,
    pub test_case_results: Vec<IndividualTestCaseResult>,
    pub total_score: u32,
    pub full_score: u32,
}

#[derive(Args, Debug)]
#[command(version)]
pub struct TestArgs {
    /// 目标类型
    #[arg(value_enum, default_value = "data")]
    pub target: Target,
}

impl StatusLabel for DisplayStatus {
    fn label(&self) -> String {
        match self {
            DisplayStatus::AC => "AC".green().to_string(),
            DisplayStatus::WA => "WA".red().to_string(),
            DisplayStatus::TLE => "TLE".blue().to_string(),
            DisplayStatus::MLE => "MLE".blue().to_string(),
            DisplayStatus::RE => "RE".bright_blue().to_string(),
            DisplayStatus::UKE => "UKE".bright_black().to_string(),
            DisplayStatus::FE => "FE".yellow().to_string(),
            DisplayStatus::CE => "CE".yellow().to_string(),
            DisplayStatus::PC(score) => format!("PC {score:.2} / 100").yellow().to_string(),
        }
    }
}

fn check_test_case(test_case: &TestCase, actual_score: u32) -> bool {
    let conditions = match &test_case.expected {
        ExpectedScore::Single(cond) => vec![cond.clone()],
        ExpectedScore::Multiple(conds) => conds.clone(),
    };

    for condition in &conditions {
        let expr = format!("{} {}", actual_score, condition);

        debug!("条件：{}", expr);

        let program = match Program::compile(&expr) {
            Ok(program) => program,
            Err(error) => {
                msg_error!("期望条件不合法：{}（{}）", condition, error);
                return false;
            }
        };

        let value = match program.execute(&CelContext::default()) {
            Ok(value) => value,
            Err(error) => {
                msg_error!("期望条件不合法：{}（{}）", condition, error);
                return false;
            }
        };

        match value {
            Value::Bool(true) => {}
            Value::Bool(false) => return false,
            _ => {
                msg_error!("期望条件不是布尔值：{}", condition);
                return false;
            }
        }
    }

    true
}

/// 解析测试者代码路径、构造运行器并完成交互配置
///
/// # Errors
///
/// 代码路径无法解析或不是文件、后缀名缺失或不受支持、运行器构造失败、交互配置
/// 缺失或不可读，以及运行器不支持交互时返回 `Err`。
fn build_tester_runner(
    day_config: &ContestDayConfig,
    problem_config: &ProblemConfig,
    test: &TestCase,
    is_sample: bool,
) -> Result<Box<dyn Runner>> {
    let path = resolve_problem_file(problem_config, &test.path)
        .with_context(|| format!("无法解析测试代码：{}", test.path))?;
    info!("文件路径：{}", path.display());

    let mut runner = build_runner(&path, day_config, problem_config)?;
    let purpose = if is_sample {
        InteractivePurpose::Sample
    } else {
        InteractivePurpose::Data
    };
    setup_interactive(problem_config, purpose, runner.as_mut())?;
    Ok(runner)
}

/// 输出并记录一个未完成评测的测试者的结果
///
/// 只留一行状态、总分 0。[`Target::Data`] 下返回 `true`（本题按未通过处理）；
/// [`Target::Sample`] 不判失败，返回 `false`。
fn record_tester_failure(
    test_name: &str,
    status: DisplayStatus,
    message: Option<String>,
    full_score: u32,
    target: Target,
) -> (ProblemTestResult, bool) {
    let result = ProblemTestResult {
        tester_name: test_name.to_string(),
        test_case_results: vec![IndividualTestCaseResult {
            test_case_id: 0,
            status,
            score: 0,
            full_score,
            time: "N/A".to_string(),
            memory: "N/A".to_string(),
            message,
        }],
        total_score: 0,
        full_score,
    };

    msg_info!(
        "{}",
        format!(
            "总得分 {}/{}",
            0.to_string().cyan().bold(),
            full_score.to_string().green().bold()
        )
        .bold()
    );

    let unmet = target == Target::Data;
    if unmet {
        msg_warn!("{}", "测试者未完成评测".bold());
    }

    (result, unmet)
}

fn write_results_to_csv(results: Vec<ProblemTestResult>, csv_path: &Path) -> Result<()> {
    let mut wtr = Writer::from_path(csv_path)?;

    wtr.write_record([
        "测试者",
        "测试点 ID",
        "状态",
        "得分",
        "满分",
        "时间",
        "空间",
        "信息",
    ])?;

    for result in &results {
        for test_case_result in &result.test_case_results {
            let message = test_case_result
                .message
                .as_deref()
                .unwrap_or("")
                .trim()
                .replace('\r', "\\r")
                .replace('\n', "\\n");
            wtr.write_record(&[
                result.tester_name.clone(),
                test_case_result.test_case_id.to_string(),
                format!("{:?}", test_case_result.status),
                test_case_result.score.to_string(),
                test_case_result.full_score.to_string(),
                test_case_result.time.clone(),
                test_case_result.memory.clone(),
                message,
            ])?;
        }

        // 追加该测试者的总分行（测试点 ID 留空，状态为 TOTAL）
        wtr.write_record(&[
            result.tester_name.clone(),
            "".to_string(),                 // 测试点 ID
            "TOTAL".to_string(),            // 状态
            result.total_score.to_string(), // 得分
            result.full_score.to_string(),  // 满分
            "".to_string(),
            "".to_string(),
            "".to_string(),
        ])?;
    }

    wtr.flush()?;
    Ok(())
}

/// 单个测试者一次评测所需的上下文
struct TesterContext<'a, 'p> {
    day_config: &'a ContestDayConfig,
    problem_config: &'a ProblemConfig,
    test_name: &'a str,
    test: &'a TestCase,
    target: Target,
    full_score: u32,
    checker: &'a dyn Checker,
    params: &'a TaskParams,
    data_items: &'a [FsTestData<'p>],
}

/// 评测单个测试者，返回汇总结果以及本题是否未通过。
fn run_tester(context: TesterContext<'_, '_>) -> Result<(ProblemTestResult, bool)> {
    let TesterContext {
        day_config,
        problem_config,
        test_name,
        test,
        target,
        full_score,
        checker,
        params,
        data_items,
    } = context;
    let is_sample = matches!(target, Target::Sample);

    info!("测试 {} 的程序", test_name);
    msg_progress!(
        "测试题目 {} 的测试 {} 的程序",
        problem_config.name.magenta(),
        test_name.cyan()
    );

    // 初始化失败（路径、语言、交互配置）-> 该测试者记一行 UKE
    let mut runner = match build_tester_runner(day_config, problem_config, test, is_sample) {
        Ok(runner) => runner,
        Err(error) => {
            let message = format!("{error:#}");
            msg_item!("UKE".bright_black().bold(), "无法完成评测");
            msg_error!("{}", message);
            return Ok(record_tester_failure(
                test_name,
                DisplayStatus::UKE,
                Some(message),
                full_score,
                target,
            ));
        }
    };

    // 编译失败 -> 记录 CE，不进入评测
    if let Err(e) = runner.prepare() {
        msg_item!("CE".yellow().bold(), "编译错误");
        msg_error!("{}", e);
        return Ok(record_tester_failure(
            test_name,
            DisplayStatus::CE,
            None,
            full_score,
            target,
        ));
    }

    // 运行所有测试点
    let mut session = TestSession::new(runner.as_mut(), checker, params.clone());
    let mut individual_results = Vec::new();
    let mut results = Vec::new();

    let case_test_pb = gctx()
        .multiprogress
        .add(ProgressBar::new(data_items.len() as u64));
    case_test_pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("  [{bar:40.magenta/blue}] {msg}")
            .unwrap()
            .progress_chars("=> "),
    );
    case_test_pb.set_message(format!("运行测试点：{}/{}", 1, data_items.len()));

    let mut case_count = 0;
    for data_item in data_items {
        case_count += 1;
        info!("运行测试点：{}", data_item.id());

        let result = session.judge(data_item)?;
        info!("测试点结果：{:?}", result.status);

        let display_status: DisplayStatus = (&result.status).into();
        let full_score = data_item.full_score();
        let earned_score = (result.score * full_score as f64).round() as u32;

        let status_str = display_status.label();

        // 非 UKE 时取判题消息首行附在结果行后；无信息则为 None
        let info_line: Option<String> = if matches!(display_status, DisplayStatus::UKE) {
            None
        } else {
            result.message.as_deref().and_then(|m| {
                let t = m.trim();
                if t.is_empty() {
                    None
                } else {
                    t.lines().next().map(|s| s.to_string())
                }
            })
        };

        individual_results.push(IndividualTestCaseResult {
            test_case_id: data_item.id(),
            status: display_status.clone(),
            score: earned_score,
            full_score,
            time: format_time(&display_status, result.time),
            memory: format_memory(&display_status, result.memory),
            message: result.message.clone(),
        });

        match info_line {
            Some(info) => msg_item!(
                status_str.clone().bold(),
                "测试点 {}  | {} | {} | {}",
                data_item.id().to_string().bold(),
                format_time(&display_status, result.time).bold(),
                format_memory(&display_status, result.memory).bold(),
                info.bright_black()
            ),
            None => msg_item!(
                status_str.clone().bold(),
                "测试点 {}  | {} | {}",
                data_item.id().to_string().bold(),
                format_time(&display_status, result.time).bold(),
                format_memory(&display_status, result.memory).bold()
            ),
        }

        // UKE 错误信息在结果行后打印
        if matches!(display_status, DisplayStatus::UKE)
            && let Some(m) = result.message.as_deref()
        {
            let t = m.trim();
            if !t.is_empty() {
                msg_error!("{}", t);
            }
        }

        case_test_pb.set_message(format!(
            "运行测试点：{}/{} | #{} {}",
            case_count + 1,
            data_items.len(),
            data_item.id(),
            status_str
        ));
        case_test_pb.inc(1);

        results.push(result);
    }

    case_test_pb.finish_and_clear();

    // 判分：按 target 选择评分策略
    let report = match target {
        Target::Data => DataPolicy.score(problem_config, data_items, &results),
        Target::Sample => SamplePolicy.score(problem_config, data_items, &results),
    };

    msg_info!("测试结果：");
    if report.groups.len() > 1 {
        for (id, group) in &report.groups {
            info!("Subtask #{} 得分 {}/{}", id, group.earned, group.full);
            msg_info!(
                "Subtask {}{} 得分 {}/{}",
                "#".bold(),
                id.to_string().bold(),
                group.earned.to_string().cyan(),
                group.full.to_string().green()
            );
        }
    }
    msg_info!(
        "{}",
        format!(
            "总得分 {}/{}",
            report.total.to_string().cyan().bold(),
            report.full_score.to_string().green().bold()
        )
        .bold()
    );

    let problem_result = ProblemTestResult {
        tester_name: test_name.to_string(),
        test_case_results: individual_results,
        total_score: report.total,
        full_score: report.full_score,
    };

    let unmet = target == Target::Data && !check_test_case(test, report.total);
    if target == Target::Data {
        if unmet {
            info!("测试 {} 不满足所有条件", test_name);
            msg_warn!("{}", "不满足所有条件".bold());
        } else {
            info!("测试 {} 通过", test_name);
        }
    }

    runner.cleanup()?;
    Ok((problem_result, unmet))
}

pub fn test_problem(
    day_config: &ContestDayConfig,
    problem_config: &ProblemConfig,
    target: Target,
    in_problem: bool,
) -> Result<bool> {
    let data_items = problem_data_items(problem_config, target);
    let is_sample = matches!(target, Target::Sample);

    // 该题目的满分：正式数据按 Subtask 满分之和，样例按数据点个数
    let full_score: u32 = if is_sample {
        data_items.len() as u32
    } else {
        problem_config
            .runtime
            .subtasks
            .values()
            .map(|g| g.max_score)
            .sum()
    };

    let params = TaskParams {
        io_mode: io_mode(problem_config),
        time_limit: Duration::try_from_secs_f64(problem_config.time_limit).map_err(|_| {
            user_error!(
                "题目 {} 的时间限制不合法：{} 秒",
                problem_config.name.magenta(),
                problem_config.time_limit
            )
        })?,
        memory_limit: problem_config.memory_limit,
    };

    // 准备 Checker(无配置时用默认 normal diff 检查器兜底)
    let checker: Box<dyn Checker> = match &problem_config.checker {
        Some(pair) => {
            let chk_config = if is_sample {
                pair.sample.as_ref().unwrap_or(&pair.data)
            } else {
                &pair.data
            };

            let source_path =
                resolve_problem_file(problem_config, &chk_config.source).map_err(|error| {
                    user_error!(
                        "无法解析题目 {} 的 Checker 源文件：{error:#}",
                        problem_config.name.magenta()
                    )
                })?;
            let deps = read_deps(problem_config, &chk_config.deps).map_err(|error| {
                user_error!(
                    "无法读取题目 {} 的 Checker 依赖：{error:#}",
                    problem_config.name.magenta()
                )
            })?;

            let cpp_checker = with_spinner(
                format!("编译 {} 题目的 Checker", problem_config.name),
                || {
                    let mut cpp_checker =
                        CppChecker::new(&source_path, &IndexMap::new(), "chk", deps).map_err(
                            |e| {
                                user_error!(
                                    "题目 {} 的 Checker 初始化失败：{}",
                                    problem_config.name.magenta(),
                                    e
                                )
                            },
                        )?;
                    cpp_checker.prepare().map_err(|e| {
                        user_error!(
                            "题目 {} 的 Checker 编译失败：{}",
                            problem_config.name.magenta(),
                            e
                        )
                    })?;
                    Ok(cpp_checker)
                },
            )?;

            Box::new(cpp_checker) as Box<dyn Checker>
        }
        None => {
            let default_binary = gctx()
                .assets_dirs
                .iter()
                .find_map(|dir| {
                    let p = dir
                        .join("checkers")
                        .join(format!("normal{}", std::env::consts::EXE_SUFFIX));
                    p.exists().then_some(p)
                })
                .context("内置 Checker 缺失（安装不完整）")?;
            Box::new(PrebuiltChecker::new(default_binary)) as Box<dyn Checker>
        }
    };

    let mut all_test_results = Vec::new();

    let tester_pb = gctx()
        .multiprogress
        .add(ProgressBar::new(problem_config.tests.len() as u64));
    tester_pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("  [{bar:40.yellow/blue}] {msg}")
            .unwrap()
            .progress_chars("=> "),
    );

    let mut tester_count = 0;
    let mut unmet = false;
    for (test_name, test) in &problem_config.tests {
        tester_count += 1;
        tester_pb.set_message(format!(
            "处理第 {}/{} 个测试者：{}",
            tester_count,
            problem_config.tests.len(),
            test_name
        ));

        let (problem_result, failed) = run_tester(TesterContext {
            day_config,
            problem_config,
            test_name: test_name.as_str(),
            test,
            target,
            full_score,
            checker: checker.as_ref(),
            params: &params,
            data_items: &data_items,
        })?;

        unmet |= failed;
        all_test_results.push(problem_result);
        tester_pb.inc(1);
    }

    if in_problem {
        tester_pb.finish_with_message("测试完成！");
    } else {
        tester_pb.finish_and_clear();
    }

    let csv_path = problem_config.path.join(match target {
        Target::Data => "result.csv",
        Target::Sample => "result-sample.csv",
    });
    if let Err(error) = write_results_to_csv(all_test_results, &csv_path) {
        user_bail!(
            "题目 {} 的测试结果写入 {} 失败：{}",
            problem_config.name.magenta(),
            csv_path.display(),
            error
        );
    }

    Ok(unmet)
}

fn test_day(day_config: &ContestDayConfig, target: Target, in_day: bool) -> Result<bool> {
    let total_problems = day_config.subconfig.len();
    let day_pb = gctx()
        .multiprogress
        .add(ProgressBar::new(total_problems as u64));
    day_pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("  [{bar:40.green/blue}] {msg}")
            .unwrap()
            .progress_chars("=> "),
    );
    let mut failed = false;
    for (idx, (_, problem_config)) in day_config.subconfig.iter().enumerate() {
        day_pb.set_message(format!("处理第 {}/{} 题", idx + 1, total_problems));
        match test_problem(day_config, problem_config, target, false) {
            Ok(problem_unmet) => failed |= problem_unmet,
            // 用户问题：跳过本题，继续其余题目
            Err(error) if is_reported(&error) => failed = true,
            Err(error) => return Err(error),
        }
        day_pb.inc(1);
    }
    if in_day {
        day_pb.finish_with_message("测试完成！");
    } else {
        day_pb.finish_and_clear();
    }
    Ok(failed)
}

pub fn main(args: TestArgs) -> Result<()> {
    let Config {
        config,
        location: current_location,
    } = gctx()
        .config
        .as_ref()
        .ok_or_else(|| user_error!("找不到配置文件"))?;

    let mut failed = false;
    match current_location {
        CurrentLocation::Problem(day_key, prob_key) => {
            let day_config = config
                .subconfig
                .get(day_key)
                .with_context(|| format!("未找到天配置：{}", day_key))?;
            let problem_config = day_config
                .subconfig
                .get(prob_key)
                .with_context(|| format!("未找到题目配置：{}", prob_key))?;
            failed |= test_problem(day_config, problem_config, args.target, true)?;
        }
        CurrentLocation::Day(day_key) => {
            let day_config = config
                .subconfig
                .get(day_key)
                .with_context(|| format!("未找到天配置：{}", day_key))?;
            failed |= test_day(day_config, args.target, true)?;
        }
        CurrentLocation::Root => {
            let total_days = config.subconfig.len();
            let day_pb = gctx()
                .multiprogress
                .add(ProgressBar::new(total_days as u64));
            day_pb.set_style(
                indicatif::ProgressStyle::default_bar()
                    .template("  [{bar:40.green/blue}] {msg}")
                    .unwrap()
                    .progress_chars("=> "),
            );
            for (day_idx, (_, day_config)) in config.subconfig.iter().enumerate() {
                day_pb.set_message(format!("处理第 {}/{} 天", day_idx + 1, total_days));
                failed |= test_day(day_config, args.target, false)?;
                day_pb.inc(1);
            }
            day_pb.finish_with_message("测试完成！");
        }
        CurrentLocation::None => user_bail!("此命令必须在工程下执行"),
    }

    if failed {
        user_bail!();
    }
    Ok(())
}

use clap::Args;
use indicatif::{ProgressBar, ProgressIterator};
use owo_colors::OwoColorize;

use crate::prelude::*;
use crate::utils::problem::{problem_data_items, read_deps, resolve_problem_file};
use crate::utils::progress::with_spinner;
use crate::utils::target::Target;
use tuack_lib::data::Data;
use tuack_lib::utils::testlib::{Validator, ValidatorResult};
use tuack_utils::data::FsTestData;
use tuack_utils::validators::cpp::CppValidator;

#[derive(Args, Debug)]
#[command(version)]
pub struct ValidateArgs {
    /// 目标类型
    #[arg(value_enum, default_value = "data")]
    pub target: Target,

    /// 校验对象，使用 `,` 和 `-` 分割 (如 1,2-3,4-10)
    #[arg(default_value = "all")]
    object: String,
}

/// 编译 [`Validator`]，失败时返回错误。
pub fn compile_validator(
    problem_config: &ProblemConfig,
    target: Target,
) -> Result<Box<dyn Validator>> {
    let is_sample = matches!(target, Target::Sample);

    let val_config = match &problem_config.validator {
        Some(pair) => {
            if is_sample {
                pair.sample.as_ref().unwrap_or(&pair.data)
            } else {
                &pair.data
            }
        }
        None => user_bail!("Validator 未配置"),
    };

    let source_path = resolve_problem_file(problem_config, &val_config.source)
        .with_context(|| format!("无法解析 Validator 源文件 {}", val_config.source))?;
    let deps = read_deps(problem_config, &val_config.deps).context("无法读取 Validator 依赖")?;

    with_spinner(
        format!("编译 {} 题目的 Validator", problem_config.name),
        || {
            let mut cpp_validator = CppValidator::new(&source_path, &IndexMap::new(), "val", deps)
                .context("Validator 初始化失败")?;
            cpp_validator.prepare().context("Validator 编译失败")?;
            Ok(Box::new(cpp_validator) as Box<dyn Validator>)
        },
    )
}

/// 校验单个题目，返回是否有输入未通过校验
fn validate_problem(
    problem_config: &ProblemConfig,
    target: Target,
    object: &str,
    in_problem: bool,
) -> Result<bool> {
    let data_items = problem_data_items(problem_config, target)?;
    let data_items =
        crate::utils::test_object::parse_test_object(object, &data_items, FsTestData::id)?;

    let validator = match compile_validator(problem_config, target) {
        Ok(v) => v,
        Err(e) => user_bail!(
            "题目 {} 的 Validator 不可用：{:#}",
            problem_config.name.magenta(),
            e
        ),
    };

    let case_pb = gctx()
        .multiprogress
        .add(ProgressBar::new(data_items.len() as u64));
    case_pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("  [{bar:40.magenta/blue}] {msg}")
            .unwrap()
            .progress_chars("=> "),
    );

    let mut failed = 0;
    for (idx, data_item) in data_items.iter().enumerate().progress_with(case_pb.clone()) {
        let result = match data_item.input() {
            Ok(mut reader) => validator.validate(&mut *reader),
            Err(e) => {
                msg_item!(
                    "FAIL".red().bold(),
                    "测试点 {} 读取输入失败：{}",
                    data_item.id().to_string().bold(),
                    e
                );
                failed += 1;
                continue;
            }
        };

        match result {
            Ok(ValidatorResult::Ok) => {
                msg_item!(
                    "OK".green(),
                    "测试点 {} 输入合法",
                    data_item.id().to_string().bold()
                );
            }
            Ok(ValidatorResult::Invalid(message)) => {
                msg_item!(
                    "FAIL".red().bold(),
                    "测试点 {} 输入不合法",
                    data_item.id().to_string().bold()
                );
                if !message.is_empty() {
                    msg_error!("{}", message);
                }
                failed += 1;
            }
            Err(e) => {
                msg_item!(
                    "FAIL".red().bold(),
                    "测试点 {} Validator 执行失败：{}",
                    data_item.id().to_string().bold(),
                    e
                );
                failed += 1;
            }
        }

        case_pb.set_message(format!("校验测试点 {}/{}", idx + 1, data_items.len()));
    }

    if in_problem {
        case_pb.finish_with_message("校验完成！");
    } else {
        case_pb.finish_and_clear();
    }

    if failed > 0 {
        msg_error!(
            "题目 {} 有 {} 个输入未通过校验",
            problem_config.name.magenta(),
            failed
        );
    }

    Ok(failed > 0)
}

/// 校验一个比赛日，返回是否有题目未通过校验
fn validate_day(
    day_config: &ContestDayConfig,
    target: Target,
    object: &str,
    in_day: bool,
) -> Result<bool> {
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
        failed |= validate_problem(problem_config, target, object, false)?;
        day_pb.inc(1);
    }
    if in_day {
        day_pb.finish_with_message("校验完成！");
    } else {
        day_pb.finish_and_clear();
    }
    Ok(failed)
}

pub fn main(args: ValidateArgs) -> Result<()> {
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
                .with_context(|| format!("未找到比赛日配置 {}", day_key))?;
            let problem_config = day_config
                .subconfig
                .get(prob_key)
                .with_context(|| format!("未找到题目配置 {}", prob_key))?;
            failed |= validate_problem(problem_config, args.target, &args.object, true)?;
        }
        CurrentLocation::Day(day_key) => {
            let day_config = config
                .subconfig
                .get(day_key)
                .with_context(|| format!("未找到比赛日配置 {}", day_key))?;
            failed = validate_day(day_config, args.target, &args.object, true)?;
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
                day_pb.set_message(format!("处理第 {}/{} 个比赛日", day_idx + 1, total_days));
                failed |= validate_day(day_config, args.target, &args.object, false)?;
                day_pb.inc(1);
            }
            day_pb.finish_with_message("校验完成！");
        }
        CurrentLocation::None => user_bail!("此命令必须在工程下执行"),
    }

    if failed {
        user_bail!();
    }
    Ok(())
}

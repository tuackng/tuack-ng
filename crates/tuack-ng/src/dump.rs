use crate::prelude::*;
use crate::utils::problem::meta;
use clap::Args;
use path_slash::PathExt;
use tuack_lib::dump::{
    DumpCase, DumpChecker, DumpConfig, DumpDocument, DumpFile, DumpProblem, DumpSample,
    DumpSubtask, ScorePolicy,
};
use tuack_utils::assets::FsAssetProvider;
use tuack_utils::utils::resolve_relative_within;
use walkdir::WalkDir;

#[derive(Args, Debug)]
#[command(version)]
pub struct DumpArgs {
    /// 列出可用导出器
    #[arg(long)]
    pub list: bool,

    /// 导出目标（内置名，或插件组件名）
    #[arg(required_unless_present = "list")]
    pub target: Option<String>,
}

/// 递归枚举 down/ 目录的附加文件
fn collect_extra_down(dir: &Path) -> Result<Vec<DumpFile>> {
    let mut files = Vec::new();
    for entry in WalkDir::new(dir).min_depth(1).sort_by_file_name() {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry.path().strip_prefix(dir)?.to_slash_lossy();
        files.push(DumpFile {
            path: PathBuf::from(format!("down/{rel}")),
        });
    }
    Ok(files)
}

/// 从 config 提取 day 级导出文档 [`DumpDocument`] 及其 [`FsAssetProvider`] 素材提供者。
fn build_dump_document(
    contest: &ContestConfig,
    day: &ContestDayConfig,
    daynum: usize,
) -> Result<(DumpDocument, FsAssetProvider)> {
    let compile = day
        .compile
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    let mut assets = FsAssetProvider::new();
    let mut problems = Vec::new();
    for (idx, (_, prob)) in day.subconfig.iter().enumerate() {
        assets.register(idx as u64, prob.path.clone());

        let data_base = prob.path.join("data");
        let data = match prob
            .runtime
            .data
            .iter()
            .map(|case| {
                let input = resolve_relative_within(&data_base, Path::new(&case.input))
                    .with_context(|| {
                        format!(
                            "无法解析题目 {} 的数据点 {} 输入文件 {}",
                            prob.name, case.id, case.input
                        )
                    })?;
                let output = resolve_relative_within(&data_base, Path::new(&case.output))
                    .with_context(|| {
                        format!(
                            "无法解析题目 {} 的数据点 {} 输出文件 {}",
                            prob.name, case.id, case.output
                        )
                    })?;
                Ok(DumpCase {
                    id: case.id,
                    score: case.score,
                    subtask: case.subtask,
                    input: Path::new("data").join(input),
                    output: Path::new("data").join(output),
                })
            })
            .collect::<Result<Vec<_>>>()
        {
            Ok(data) => data,
            Err(error) => user_bail!("{error:#}"),
        };

        let subtasks = prob
            .runtime
            .subtasks
            .iter()
            .map(|(k, st)| {
                (
                    *k,
                    DumpSubtask {
                        items: st.items.clone(),
                        max_score: st.max_score,
                        policy: match st.policy {
                            tuack_config::ScorePolicy::Sum => ScorePolicy::Sum,
                            tuack_config::ScorePolicy::Min => ScorePolicy::Min,
                            tuack_config::ScorePolicy::Max => ScorePolicy::Max,
                        },
                    },
                )
            })
            .collect();

        let sample_base = prob.path.join("sample");
        let samples = match prob
            .samples
            .iter()
            .map(|s| {
                let input_path = s.input_path();
                let output_path = s.output_path();
                let input = resolve_relative_within(&sample_base, Path::new(&input_path))
                    .with_context(|| {
                        format!(
                            "无法解析题目 {} 的样例 {} 输入文件 {}",
                            prob.name, s.id, input_path
                        )
                    })?;
                let output = resolve_relative_within(&sample_base, Path::new(&output_path))
                    .with_context(|| {
                        format!(
                            "无法解析题目 {} 的样例 {} 输出文件 {}",
                            prob.name, s.id, output_path
                        )
                    })?;
                Ok(DumpSample {
                    input: Path::new("sample").join(input),
                    output: Path::new("sample").join(output),
                })
            })
            .collect::<Result<Vec<_>>>()
        {
            Ok(samples) => samples,
            Err(error) => user_bail!("{error:#}"),
        };

        let checker = prob.checker.as_ref().map(|c| DumpChecker {
            source: PathBuf::from(&c.data.source),
            deps: c.data.deps.iter().map(PathBuf::from).collect(),
        });

        let extra_dir = prob.path.join("down");
        let extra_down = if extra_dir.exists() {
            collect_extra_down(&extra_dir)?
        } else {
            Vec::new()
        };

        problems.push(DumpProblem {
            idx: idx as u64,
            meta: meta(prob, day),
            data,
            subtasks,
            samples,
            extra_down,
            checker,
        });
    }

    Ok((
        DumpDocument {
            config: DumpConfig {
                contest_name: contest.name.clone(),
                day_name: day.name.clone(),
                dayidx: daynum,
                compile,
            },
            problems,
        },
        assets,
    ))
}

fn dump_main(
    contest: &ContestConfig,
    day: &ContestDayConfig,
    daynum: usize,
    target: &str,
) -> Result<()> {
    let (doc, assets) = build_dump_document(contest, day, daynum)?;
    let dump_dir = day.path.join("dump");

    let tmp = Arc::new(
        tempfile::Builder::new()
            .prefix("tuack-ng-dump-")
            .tempdir()
            .context("创建临时目录失败")?,
    );

    let dumper = gctx().plugins.dumper(target, tmp.clone())?;

    let (files, warnings) = match dumper.dump(&doc, Box::new(assets)) {
        Ok(result) => result,
        Err(e) => {
            msg_error!("导出失败：{:?}", e);
            msg_info!("保留临时目录 {} 以供调试", tmp.path().display());
            // 故意泄漏临时目录，使其不被自动清理，便于事后调试
            std::mem::forget(tmp.clone());
            user_bail!();
        }
    };

    for warning in &warnings {
        msg_warn!("{}", warning);
    }

    let out_dir = dump_dir.join(target);
    if out_dir.exists() {
        fs::remove_dir_all(&out_dir)?;
    }
    fs::create_dir_all(&out_dir)?;

    if let Err(e) = crate::utils::filesystem::write_outputs(&out_dir, files) {
        msg_error!("写入导出结果失败：{:?}", e);
        msg_info!("保留临时目录 {} 以供调试", tmp.path().display());
        // 故意泄漏临时目录，使其不被自动清理，便于事后调试
        std::mem::forget(tmp.clone());
        user_bail!();
    }
    msg_info!("导出完成，输出目录 {}", out_dir.display());

    Ok(())
}

pub fn main(args: DumpArgs) -> Result<()> {
    if args.list {
        msg!("可用导出器：");
        for name in gctx().plugins.dumper_names() {
            msg!("  {}", name);
        }
        return Ok(());
    }
    let target = args.target.as_deref().expect("clap 保证存在 target");
    if gctx().config.is_none() {
        user_bail!("没有有效的配置文件");
    }
    let config = gctx().config.clone().unwrap();
    match config.location {
        CurrentLocation::None => user_bail!("此命令必须在工程下执行"),
        CurrentLocation::Problem(_, _) => user_bail!("此命令不能在题目下执行"),
        CurrentLocation::Day(day) => {
            dump_main(
                &config.config,
                config.config.subconfig.get(&day).unwrap(),
                1,
                target,
            )?;
        }
        CurrentLocation::Root => {
            let mut failed = false;
            for (idx, (_, day_config)) in config.config.subconfig.iter().enumerate() {
                match dump_main(&config.config, day_config, idx + 1, target) {
                    Ok(()) => {}
                    // 用户问题已在失败点输出，继续处理其余比赛日
                    Err(e) if is_reported(&e) => failed = true,
                    // 未报告的错误 = 未处理的自身问题：立即中止
                    Err(e) => {
                        return Err(e).context(format!("比赛日 {} 导出失败", day_config.name));
                    }
                }
            }
            if failed {
                user_bail!();
            }
        }
    }

    Ok(())
}

//! Tuack-NG 命令行入口：定义子命令并分发到各模块。
//!
//! 退出码约定：运行正常为 0；遇到了用户有关的问题（相关问题已在函数内输出）为 1；
//! 遇到了程序自身的问题为 2，错误由顶层输出。
//!
//! 日志与 panic 策略随构建类型变化：调试构建下详细日志默认开启（`--silent` 关闭），
//! panic 原样抛出；发布构建下详细日志默认关闭（`--verbose` 打开）。

use std::process::ExitCode;

use crate::conf::ConfArgs;
use crate::dmk::DmkArgs;
use crate::doc::DocArgs;
use crate::dump::DumpArgs;
use crate::generate::GenArgs;
use crate::plugin::PluginArgs;
use crate::prelude::*;
use crate::ren::RenArgs;
use crate::test::TestArgs;
use crate::validate::ValidateArgs;
use clap::{Parser, Subcommand};
use clap_i18n_richformatter::clap_i18n;

mod conf;
mod context;
mod develop;
mod dmk;
mod doc;
mod dump;
mod errors;
mod generate;
mod init;
mod plugin;
mod prelude;
mod ren;
mod test;
mod utils;
mod validate;

#[derive(Debug, Parser)]
#[clap_i18n]
#[command(
    version,
    about = "Tuack-NG",
    disable_help_subcommand = true,
    after_long_help = exit_codes_help()
)]
struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
    #[arg(short, long, global = true)]
    /// 详细模式
    verbose: bool,
    #[cfg(debug_assertions)]
    #[arg(long, global = true)]
    /// 静默模式，无视详细（仅调试构建）
    silent: bool,
}

/// 帮助末尾的退出码说明
fn exit_codes_help() -> String {
    format!(
        "{}\n  {}  表示正常\n  {}  表示用户相关的问题\n  {}  程序自身的问题（Bug 或安装不完整）",
        "退出码：".underline().bold(),
        "0".bold(),
        "1".bold(),
        "2".bold(),
    )
}

#[derive(Subcommand, Debug)]
#[command(infer_subcommands = false)]
enum Commands {
    /// 渲染题面
    Ren(RenArgs),
    /// 生成工程文件夹
    Gen(GenArgs),
    /// 使用题解代码测试
    Test(TestArgs),
    /// 批量修改配置文件
    Conf(ConfArgs),
    /// 生成数据
    Dmk(DmkArgs),
    /// 校验输入数据
    Validate(ValidateArgs),
    /// 导出到评测系统
    Dump(DumpArgs),
    /// 文档相关
    Doc(DocArgs),
    /// 开发工具
    Develop(develop::DevelopArgs),
    /// 插件管理
    Plugin(PluginArgs),
}

/// 输出无子命令时的欢迎信息
fn print_welcome() {
    let title = "欢迎使用 Tuack-NG，很高兴与你相遇！❤️".bold().to_string();
    let docs = "https://docs.tuack-ng.ink/app/project/overview.html"
        .bold()
        .underline()
        .to_string();
    let from_docs = "先阅读文档".bold().to_string();
    let most_used = "最常使用".bold().to_string();
    let community = "https://docs.tuack-ng.ink/community"
        .bold()
        .underline()
        .to_string();
    let gen_cmd = "tuack-ng gen <contest|day|problem|...> <名称>"
        .cyan()
        .bold()
        .to_string();
    let ren = "tuack-ng ren <模板>".cyan().bold().to_string();
    let test = "tuack-ng test".cyan().bold().to_string();
    let dmk = "tuack-ng dmk <data|sample> <gen|regen|reset> [对象]"
        .cyan()
        .bold()
        .to_string();
    let validate = "tuack-ng validate".cyan().bold().to_string();
    let dump = "tuack-ng dump <目标>".cyan().bold().to_string();
    let help = "tuack-ng --help".cyan().bold().to_string();

    println!(
        r#"{title}

如果你没有使用过 Tuack-NG，可以{from_docs} <{docs}>。

这里有一些 Tuack-NG 中{most_used}的命令：

- 新建一个工程：{gen_cmd}
- 渲染题面：    {ren}
- 测试题解：    {test}
- 生成数据：    {dmk}
- 校验数据：    {validate}
- 导出：        {dump}

可以运行 {help} 来了解这些命令的具体用法。

如果有什么疑问，欢迎加入我们的讨论社区 <{community}>。

祝你好运！"#
    );
}
fn tuack_ng(cli: Cli) -> Result<()> {
    if cli.command.is_none() {
        print_welcome();
        return Ok(());
    }

    init::init(
        &{
            #[cfg(debug_assertions)]
            {
                !cli.silent
            }
            #[cfg(not(debug_assertions))]
            {
                cli.verbose
            }
        },
        &cli,
    )?;
    info!("booting up");
    if let Ok(cwd) = dunce::canonicalize(Path::new(".")) {
        debug!("当前目录 {}", cwd.display());
    }

    match cli.command.expect("已在入口检查子命令") {
        Commands::Ren(args) => ren::main(args),
        Commands::Gen(args) => generate::main(args),
        Commands::Test(args) => test::main(args),
        Commands::Conf(args) => conf::main(args),
        Commands::Dmk(args) => dmk::main(args),
        Commands::Validate(args) => validate::main(args),
        Commands::Dump(args) => dump::main(args),
        Commands::Doc(args) => doc::main(args),
        Commands::Develop(args) => develop::main(args),
        Commands::Plugin(args) => plugin::main(args),
    }
}

/// 输出未处理的自身问题
fn report(error: &anyhow::Error) {
    if cfg!(debug_assertions) {
        msg_error!("程序执行出错：{:?}", error);
    } else {
        msg_error!("程序执行出错：{:#}", error);
    }
}

fn main() -> ExitCode {
    let cli = match Cli::parse_i18n() {
        Ok(cli) => cli,
        Err(error) => {
            let _ = error.print();
            // help/version 正常输出；用法错误属于用户问题（1）
            return if error.use_stderr() {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            };
        }
    };

    match tuack_ng(cli) {
        Ok(()) => ExitCode::SUCCESS,
        // 用户问题已在抛出点输出
        Err(error) if is_reported(&error) => ExitCode::from(1),
        // 未报告的错误 = 未处理的自身问题
        Err(error) => {
            report(&error);
            ExitCode::from(2)
        }
    }
}
